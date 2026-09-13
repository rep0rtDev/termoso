package com.termoso.android.ui.terminal

import android.view.inputmethod.InputMethodManager
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.getSystemService
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import com.termoso.core.CursorStyle
import com.termoso.core.KeyMods
import com.termoso.core.SpecialKey
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.abs

/** Position of a cell under a touch. */
data class CellPoint(val row: Int, val col: Int)

/**
 * Terminal surface: a Compose [Canvas] fed by packed frames from Rust, plus a
 * hidden [TerminalInputView] that owns the soft keyboard. The Rust grid is
 * resized to whatever number of whole cells fits the measured size, so the
 * remote always sees the true geometry.
 */
@Composable
fun TerminalView(
    session: TerminalSession,
    controller: TerminalController,
    fontSizeSp: Int,
    fontFamily: String,
    cursorBlink: Boolean,
    cursorStyle: String,
    modifier: Modifier = Modifier,
    onFrame: (CellGrid) -> Unit = {},
    onTap: () -> Unit = {},
    onLongPress: (CellPoint, Offset) -> Unit = { _, _ -> },
    onZoom: (Int) -> Unit = {},
) {
    val density = LocalDensity.current
    val context = LocalContext.current
    val metrics = remember(fontSizeSp, fontFamily, density) {
        CellMetrics(with(density) { fontSizeSp.sp.toPx() }, terminalTypefaces(context, fontFamily))
    }
    val renderer = remember { TerminalRenderer() }

    var grid by remember(session) { mutableStateOf<CellGrid?>(null) }
    var size by remember { mutableStateOf(IntSize.Zero) }
    var focused by remember { mutableStateOf(false) }
    var blinkOn by remember { mutableStateOf(true) }

    val tick by session.frameTick.collectAsStateWithLifecycle()

    LaunchedEffect(session, tick) {
        val frame = withContext(Dispatchers.Default) { session.rust.frame() }
        val next = CellGrid(frame)
        grid = next
        blinkOn = true
        onFrame(next)
    }

    LaunchedEffect(size, metrics) {
        if (size == IntSize.Zero) return@LaunchedEffect
        val cols = (size.width / metrics.width).toInt().coerceIn(2, 500)
        val rows = (size.height / metrics.height).toInt().coerceIn(1, 300)
        val current = grid
        if (current == null || current.cols != cols || current.rows != rows) {
            withContext(Dispatchers.Default) { session.rust.resize(cols.toUShort(), rows.toUShort()) }
        }
    }

    LaunchedEffect(cursorBlink, focused) {
        if (!cursorBlink || !focused) {
            blinkOn = true
            return@LaunchedEffect
        }
        var last = 0L
        while (true) {
            withFrameNanos { now ->
                if (now - last > 530_000_000L) {
                    blinkOn = !blinkOn
                    last = now
                }
            }
        }
    }

    val override = when (cursorStyle) {
        "underline" -> CursorStyle.UNDERLINE
        "beam" -> CursorStyle.BEAM
        else -> null
    }
    val background = grid?.frame?.background?.toInt()?.let { Color(it or (0xFF shl 24)) } ?: Color(0xFF0D1117)

    val currentTap by rememberUpdatedState(onTap)
    val currentLongPress by rememberUpdatedState(onLongPress)
    val currentZoom by rememberUpdatedState(onZoom)
    val currentMetrics by rememberUpdatedState(metrics)
    val currentGrid by rememberUpdatedState(grid)

    Box(
        modifier
            .background(background)
            .onSizeChanged { size = it }
            .pointerInput(session) {
                detectTapGestures(
                    onTap = {
                        controller.inputView?.let { v ->
                            v.requestFocus()
                            context.getSystemService<InputMethodManager>()?.showSoftInput(v, 0)
                        }
                        currentTap()
                    },
                    onLongPress = { offset ->
                        val cell = CellPoint(
                            row = (offset.y / currentMetrics.height).toInt(),
                            col = (offset.x / currentMetrics.width).toInt(),
                        )
                        currentLongPress(cell, offset)
                    },
                )
            }
            .pointerInput(session) {
                awaitEachGesture {
                    awaitFirstDown(requireUnconsumed = false)
                    var scrollAcc = 0f
                    var zoomAcc = 1f
                    var multi = false
                    do {
                        val event = awaitPointerEvent()
                        val pressed = event.changes.count { it.pressed }
                        if (pressed >= 2) {
                            multi = true
                            zoomAcc *= event.calculateZoom()
                            if (zoomAcc > 1.12f) {
                                currentZoom(1)
                                zoomAcc = 1f
                            } else if (zoomAcc < 0.89f) {
                                currentZoom(-1)
                                zoomAcc = 1f
                            }
                            event.changes.forEach { it.consume() }
                        } else if (!multi) {
                            scrollAcc += event.calculatePan().y
                            val cellH = currentMetrics.height
                            val lines = (scrollAcc / cellH).toInt()
                            if (lines != 0) {
                                scrollAcc -= lines * cellH
                                controller.scrollBy(lines, currentGrid?.frame?.altScreen == true)
                                event.changes.forEach { it.consume() }
                            }
                        }
                    } while (event.changes.any { it.pressed })
                }
            },
    ) {
        Canvas(Modifier.fillMaxSize()) {
            val g = grid ?: return@Canvas
            renderer.draw(
                canvas = drawContext.canvas.nativeCanvas,
                grid = g,
                metrics = metrics,
                cursorVisible = blinkOn,
                cursorOverride = override,
                focused = focused,
            )
        }
        AndroidView(
            factory = { ctx ->
                TerminalInputView(ctx).also { v ->
                    v.setOnFocusChangeListener { _, has -> focused = has }
                    controller.inputView = v
                }
            },
            update = { v ->
                v.onText = { text -> controller.sendText(text) }
                v.onTextWithMods = { text, mods -> controller.sendText(text, mods) }
                v.onKey = { key, mods -> controller.sendKey(key, mods) }
            },
            modifier = Modifier.size(1.dp),
        )
    }
}

/** Scroll-position helper for the scroll-to-bottom affordance. */
fun CellGrid.scrolledLines(): Int = frame.displayOffset.toInt()

/**
 * Input funnel for one session: applies the sticky modifiers from the key
 * panel and forwards everything to Rust, which does the actual key encoding.
 */
class TerminalController(private val session: TerminalSession) {
    var inputView: TerminalInputView? = null

    var ctrl by mutableStateOf(false)
    var alt by mutableStateOf(false)
    var shift by mutableStateOf(false)

    private fun takeMods(extra: KeyMods): KeyMods {
        val m = KeyMods(ctrl = ctrl || extra.ctrl, alt = alt || extra.alt, shift = shift || extra.shift)
        ctrl = false
        alt = false
        shift = false
        return m
    }

    fun sendText(text: String, extra: KeyMods = NONE) {
        val mods = takeMods(extra)
        session.rust.sendText(text, KeyMods(ctrl = mods.ctrl, alt = mods.alt, shift = false))
        session.rust.scrollToBottom()
    }

    fun sendKey(key: SpecialKey, extra: KeyMods = NONE) {
        session.rust.sendKey(key, takeMods(extra))
        session.rust.scrollToBottom()
    }

    /** Text that bypasses sticky modifiers (hidden input, snippets). */
    fun sendRaw(text: String, enter: Boolean) {
        session.rust.sendText(text, NONE)
        if (enter) session.rust.sendKey(SpecialKey.ENTER, NONE)
        session.rust.scrollToBottom()
    }

    fun paste(text: String) {
        session.rust.paste(text)
        session.rust.scrollToBottom()
    }

    /** Positive = towards history. On the alternate screen it becomes arrow keys. */
    fun scrollBy(lines: Int, altScreen: Boolean) {
        if (altScreen) {
            val key = if (lines > 0) SpecialKey.UP else SpecialKey.DOWN
            repeat(abs(lines)) { session.rust.sendKey(key, NONE) }
        } else {
            session.rust.scroll(lines)
        }
    }

    fun scrollToBottom() = session.rust.scrollToBottom()

    fun visibleText(): String = session.rust.visibleText().joinToString("\n").trimEnd()

    private companion object {
        val NONE = KeyMods(ctrl = false, alt = false, shift = false)
    }
}
