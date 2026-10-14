package com.termoso.android.ui.terminal

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Live thumbnail of a running session, as on the Termius Connections tab: the
 * current frame drawn small (no cursor, no input) and refreshed with every
 * render tick. Clip and size it from the caller.
 */
@Composable
fun TerminalPreview(session: TerminalSession, fontFamily: String, modifier: Modifier = Modifier, fontSizeSp: Int = 6) {
    val density = LocalDensity.current
    val context = LocalContext.current
    val metrics = remember(fontFamily, density) {
        CellMetrics(with(density) { fontSizeSp.sp.toPx() }, terminalTypefaces(context, fontFamily))
    }
    val renderer = remember { TerminalRenderer() }
    val tick by session.frameTick.collectAsStateWithLifecycle()
    var grid by remember(session) { mutableStateOf<CellGrid?>(null) }
    LaunchedEffect(session, tick) {
        grid = withContext(Dispatchers.Default) { CellGrid(session.rust.frame()) }
    }
    val background = grid?.frame?.background?.toInt()?.let { Color(it or (0xFF shl 24)) } ?: Color(0xFF0D1117)
    Canvas(modifier.background(background)) {
        val g = grid ?: return@Canvas
        renderer.draw(
            canvas = drawContext.canvas.nativeCanvas,
            grid = g,
            metrics = metrics,
            cursorVisible = false,
            cursorOverride = null,
            focused = false,
        )
    }
}
