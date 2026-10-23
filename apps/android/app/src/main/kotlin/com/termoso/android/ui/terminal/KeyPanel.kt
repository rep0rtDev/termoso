package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.DataObject
import androidx.compose.material.icons.filled.EditNote
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material.icons.filled.Password
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.termoso.android.R
import com.termoso.core.KeyMods
import com.termoso.core.SpecialKey

private val NO_MODS = KeyMods(ctrl = false, alt = false, shift = false)

/** Something the key panel can send. */
sealed interface PanelKey {
    val label: String

    data class Special(override val label: String, val key: SpecialKey, val mods: KeyMods = NO_MODS) : PanelKey
    data class Text(override val label: String, val text: String = label, val mods: KeyMods = NO_MODS) : PanelKey
    data class Modifier(override val label: String, val which: StickyMod) : PanelKey
}

enum class StickyMod { CTRL, ALT, SHIFT }

/** Send [key] through [controller]; sticky modifiers from the panel still apply on top. */
fun TerminalController.press(key: PanelKey) {
    when (key) {
        is PanelKey.Special -> sendKey(key.key, key.mods)
        is PanelKey.Text -> sendText(key.text, key.mods)
        is PanelKey.Modifier -> when (key.which) {
            StickyMod.CTRL -> ctrl = !ctrl
            StickyMod.ALT -> alt = !alt
            StickyMod.SHIFT -> shift = !shift
        }
    }
}

// Measured from Termius Android at 2.625 px/dp: the strip is a flat 40 dp row of glyphs
// ~40 dp apart (no tiles); the grid has 30 dp tiles, 7 dp apart, in two 4-tile hand
// blocks separated by a tile-wide gutter, 27 dp from the sides; the bottom tab bar is
// 50 dp with the active tab a filled 40 dp square.
private val KeyShape = RoundedCornerShape(8.dp)
private val KeyGap = 6.dp
private val GridGap = 7.dp
private val GridRowGap = 7.dp
private val GridSidePadding = 27.dp
private val GridGutter = 35.dp
private val StripKeyHeight = 36.dp
private val GridKeyHeight = 30.dp
private val TabBarHeight = 50.dp
/** Grid height when the keyboard has never been shown yet (no measured height to match). */
private val GridDefaultHeight = 272.dp
private val BarHeight = 40.dp

private val ChromePadding = PaddingValues(horizontal = 10.dp, vertical = 2.dp)

/** Keys the compact strip always offers, scrolling sideways past the edge. */
private val stripKeys: List<PanelKey> = listOf(
    PanelKey.Special("shift+tab", SpecialKey.TAB, KeyMods(ctrl = false, alt = false, shift = true)),
    PanelKey.Special("esc", SpecialKey.ESCAPE),
    PanelKey.Modifier("ctrl", StickyMod.CTRL),
    PanelKey.Special("←", SpecialKey.LEFT),
    PanelKey.Special("↑", SpecialKey.UP),
    PanelKey.Special("↓", SpecialKey.DOWN),
    PanelKey.Special("→", SpecialKey.RIGHT),
    PanelKey.Modifier("alt", StickyMod.ALT),
    PanelKey.Special("tab", SpecialKey.TAB),
    PanelKey.Text("-"),
    PanelKey.Text("|"),
    PanelKey.Text("/"),
    PanelKey.Text("~"),
    PanelKey.Text(":"),
)

/**
 * Extra-keys panel above the soft keyboard, laid out like the Termius iOS one.
 *
 * Compact: one strip — grid toggle, Ask AI, hidden password input, a
 * sideways-scrolling row of the everyday keys, paste and the keyboard toggle. Expanded: Customize/Password shortcuts,
 * a scrollable grid of [rows] (built-in groups or whatever the user arranged in
 * Settings → Customize keys) as rounded tiles, and a bottom bar with the tool
 * sheets (snippets, history & themes, Ask AI) and the keyboard toggle.
 * Ctrl/Alt/Shift are sticky: they apply to the next key, then release. With
 * [collapsed] (physical keyboard attached) only a thin handle remains.
 */
@Composable
fun KeyPanel(
    controller: TerminalController,
    rows: List<List<PanelKey>>,
    expanded: Boolean,
    collapsed: Boolean,
    imeShown: Boolean,
    onToggleExpanded: () -> Unit,
    onToggleCollapsed: () -> Unit,
    onToggleIme: () -> Unit,
    onHiddenInput: () -> Unit,
    onSnippets: () -> Unit,
    onAskAi: () -> Unit,
    onPanel: () -> Unit,
    onPaste: () -> Unit,
    onCustomize: () -> Unit,
    /** Whether this session can take files; shows the paperclip. */
    canAttach: Boolean = false,
    onAttach: () -> Unit = {},
    onCompose: () -> Unit = {},
    onVoice: () -> Unit = {},
    onKeyPressed: () -> Unit,
    /** Height of the soft keyboard this grid stands in for; matching it keeps the terminal from jumping. */
    gridHeight: androidx.compose.ui.unit.Dp? = null,
    modifier: Modifier = Modifier,
) {
    fun press(key: PanelKey) {
        onKeyPressed()
        controller.press(key)
    }

    fun active(key: PanelKey): Boolean = key is PanelKey.Modifier && when (key.which) {
        StickyMod.CTRL -> controller.ctrl
        StickyMod.ALT -> controller.alt
        StickyMod.SHIFT -> controller.shift
    }

    val chrome = MaterialTheme.colorScheme.surfaceContainer
    Column(modifier.background(chrome)) {
        if (collapsed) {
            Row(
                Modifier.fillMaxWidth().height(30.dp).padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(
                    Icons.Filled.Keyboard,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(16.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    stringResource(R.string.physical_keyboard),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.weight(1f),
                )
                IconKey(Icons.Filled.KeyboardArrowUp, stringResource(R.string.show_key_panel), Modifier.width(44.dp).height(26.dp), onClick = onToggleCollapsed)
            }
            return@Column
        }
        if (!expanded) {
            // Termius strip: tools first (AI, hidden input, attach, paste), then the keys,
            // all flat glyphs that scroll sideways. Grid/keyboard toggles live in the session bar.
            LazyRow(
                Modifier.fillMaxWidth().height(BarHeight),
                verticalAlignment = Alignment.CenterVertically,
                contentPadding = ChromePadding,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                item("ai") { IconKey(Icons.Filled.AutoAwesome, stringResource(R.string.ask_ai), Modifier.width(40.dp), flat = true) { onKeyPressed(); onAskAi() } }
                item("pw") { IconKey(Icons.Filled.Key, stringResource(R.string.password), Modifier.width(40.dp), flat = true) { onKeyPressed(); onHiddenInput() } }
                if (canAttach) {
                    item("attach") { IconKey(Icons.Filled.AttachFile, stringResource(R.string.send_file), Modifier.width(40.dp), flat = true) { onKeyPressed(); onAttach() } }
                }
                item("paste") { IconKey(Icons.Filled.ContentPaste, stringResource(R.string.paste_2), Modifier.width(40.dp), flat = true) { onKeyPressed(); onPaste() } }
                stripKeys.forEach { key ->
                    item(key = KeyActions.encode(key)) {
                        TextKey(key, active(key), Modifier.widthIn(min = 40.dp), height = StripKeyHeight, flat = true) { press(it) }
                    }
                }
                item("voice") { IconKey(Icons.Filled.Mic, stringResource(R.string.voice_input), Modifier.width(40.dp), flat = true) { onKeyPressed(); onVoice() } }
                item("compose") { IconKey(Icons.Filled.EditNote, stringResource(R.string.compose_message), Modifier.width(40.dp), flat = true) { onKeyPressed(); onCompose() } }
                item("snippets") { IconKey(Icons.Filled.DataObject, stringResource(R.string.snippets), Modifier.width(40.dp), flat = true) { onKeyPressed(); onSnippets() } }
            }
            return@Column
        }
        Column(Modifier.fillMaxWidth().padding(top = 10.dp, start = GridSidePadding, end = GridSidePadding)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(GridGutter)) {
                WideKey(Icons.Filled.Settings, stringResource(R.string.customize), Modifier.weight(1f)) { onKeyPressed(); onCustomize() }
                Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(GridGap)) {
                    IconKey(Icons.Filled.AutoAwesome, stringResource(R.string.ask_ai), Modifier.weight(1f), tile = true) { onKeyPressed(); onAskAi() }
                    IconKey(Icons.Filled.Key, stringResource(R.string.password), Modifier.weight(1f), tile = true) { onKeyPressed(); onHiddenInput() }
                    IconKey(Icons.Filled.AttachFile, stringResource(R.string.send_file), Modifier.weight(1f), tile = true, enabled = canAttach) { onKeyPressed(); onAttach() }
                    IconKey(Icons.Filled.ContentPaste, stringResource(R.string.paste_2), Modifier.weight(1f), tile = true) { onKeyPressed(); onPaste() }
                }
            }
            Spacer(Modifier.height(GridRowGap))
            Column(
                Modifier
                    .fillMaxWidth()
                    .height((gridHeight ?: GridDefaultHeight) + BarHeight - GridKeyHeight - GridRowGap - 10.dp - TabBarHeight)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(GridRowGap),
            ) {
                rows.forEach { row -> KeyRow(row, ::active, ::press) }
                Spacer(Modifier.height(2.dp))
            }
        }
        Row(
            Modifier.fillMaxWidth().height(TabBarHeight).padding(horizontal = GridSidePadding, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconKey(Icons.Filled.GridView, stringResource(R.string.hide_keys), Modifier.weight(1f), selected = true, onClick = {})
            IconKey(Icons.Filled.DataObject, stringResource(R.string.snippets), Modifier.weight(1f), flat = true) { onKeyPressed(); onSnippets() }
            IconKey(Icons.Filled.History, stringResource(R.string.history_and_themes), Modifier.weight(1f), flat = true) { onKeyPressed(); onPanel() }
            IconKey(Icons.Filled.EditNote, stringResource(R.string.compose_message), Modifier.weight(1f), flat = true) { onKeyPressed(); onCompose() }
            Box(Modifier.width(1.dp).height(22.dp).background(MaterialTheme.colorScheme.outlineVariant))
            IconKey(
                if (imeShown) Icons.Filled.KeyboardHide else Icons.Filled.Keyboard,
                stringResource(R.string.toggle_keyboard),
                Modifier.weight(1f),
                flat = true,
                onClick = onToggleIme,
            )
        }
    }
}

/**
 * One grid row. Eight keys split 4 | 4 with a wider gutter in the middle, the
 * way the iOS grid reads as two hands; anything else spreads evenly.
 */
@Composable
private fun KeyRow(row: List<PanelKey>, active: (PanelKey) -> Boolean, press: (PanelKey) -> Unit) {
    if (row.size > 8) {
        // Long custom rows wrap into 8-key lines instead of shrinking into slivers.
        Column(verticalArrangement = Arrangement.spacedBy(GridRowGap)) {
            row.chunked(8).forEach { KeyRow(it, active, press) }
        }
        return
    }
    // 7 keys: 3 | 4 with the first key double width (Termius' shift+tab esc ctrl | arrows).
    val split = when (row.size) { 8 -> 4; 7 -> 3; 2 -> 1; else -> -1 }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(GridGap)) {
        row.forEachIndexed { i, key ->
            if (i == split) Spacer(Modifier.width(GridGutter - GridGap * 2))
            val weight = if (row.size == 7 && i == 0) 2f else 1f
            TextKey(key, active(key), Modifier.weight(weight), height = GridKeyHeight) { press(it) }
        }
    }
}

@Composable
private fun keyContainer(active: Boolean): Color =
    if (active) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f) else MaterialTheme.colorScheme.surfaceContainerHigh

@Composable
private fun TextKey(
    key: PanelKey,
    active: Boolean,
    modifier: Modifier = Modifier,
    height: androidx.compose.ui.unit.Dp = GridKeyHeight,
    /** Strip style: no tile, just the label (Termius-like). */
    flat: Boolean = false,
    onPress: (PanelKey) -> Unit,
) {
    val arrow = when (key.label) {
        "←" -> Icons.AutoMirrored.Filled.ArrowBack
        "→" -> Icons.AutoMirrored.Filled.ArrowForward
        "↑" -> Icons.Filled.ArrowUpward
        "↓" -> Icons.Filled.ArrowDownward
        else -> null
    }
    val twoLine = key.label.contains('\n')
    val single = when {
        twoLine -> 11.sp
        flat -> 15.sp
        key.label.length <= 3 -> 14.sp
        key.label.length <= 4 -> 12.sp
        key.label.length <= 9 -> 11.sp
        else -> 10.sp
    }
    Box(
        modifier
            .height(height)
            .clip(KeyShape)
            .background(if (flat && !active) Color.Transparent else keyContainer(active))
            .clickable { onPress(key) }
            .padding(horizontal = if (flat) 8.dp else 2.dp),
        contentAlignment = Alignment.Center,
    ) {
        if (arrow != null) {
            Icon(
                arrow,
                contentDescription = key.label,
                tint = if (flat) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(if (flat) 22.dp else 18.dp),
            )
            return@Box
        }
        Text(
            key.label,
            fontSize = single,
            lineHeight = if (twoLine) 12.sp else 16.sp,
            textAlign = TextAlign.Center,
            maxLines = if (twoLine) 2 else 1,
            softWrap = twoLine,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            color = when {
                active -> MaterialTheme.colorScheme.primary
                flat -> MaterialTheme.colorScheme.onSurface
                else -> MaterialTheme.colorScheme.onSurfaceVariant
            },
        )
    }
}

@Composable
private fun WideKey(icon: ImageVector, label: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Row(
        modifier
            .height(GridKeyHeight)
            .clip(KeyShape)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .clickable(onClick = onClick),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.Center,
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
        Spacer(Modifier.width(8.dp))
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun MenuAction(icon: ImageVector, label: String, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        leadingIcon = { Icon(icon, contentDescription = null, Modifier.size(20.dp)) },
        onClick = onClick,
    )
}

@Composable
private fun IconKey(
    icon: ImageVector,
    description: String,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    flat: Boolean = false,
    /** Grid style: same rounded tile as a text key. */
    tile: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val bg = when {
        selected -> MaterialTheme.colorScheme.primary
        tile -> MaterialTheme.colorScheme.surfaceContainerHigh
        else -> Color.Transparent
    }
    if (selected) {
        Box(modifier, contentAlignment = Alignment.Center) {
            Box(
                Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).background(bg).clickable(onClick = onClick),
                contentAlignment = Alignment.Center,
            ) {
                Icon(icon, contentDescription = description, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(22.dp))
            }
        }
        return
    }
    Box(
        modifier
            .height(if (tile) GridKeyHeight else StripKeyHeight)
            .clip(KeyShape)
            .background(bg)
            .clickable(enabled = enabled, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            icon,
            contentDescription = description,
            tint = when {
                selected -> MaterialTheme.colorScheme.onPrimary
                !enabled -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f)
                flat -> MaterialTheme.colorScheme.primary
                else -> MaterialTheme.colorScheme.onSurface
            },
            modifier = Modifier.size(if (flat) 22.dp else 20.dp),
        )
    }
}
