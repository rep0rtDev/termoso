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
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AutoAwesome
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.DataObject
import androidx.compose.material.icons.filled.GridView
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.Password
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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

private val KeyShape = RoundedCornerShape(10.dp)
private val KeyGap = 6.dp
private val StripKeyHeight = 40.dp
private val GridKeyHeight = 44.dp
private val GridMaxHeight = 232.dp
private val BarHeight = 46.dp
private val ChromePadding = PaddingValues(horizontal = 8.dp, vertical = 5.dp)

/** Keys the compact strip always offers, scrolling sideways past the edge. */
private val stripKeys: List<PanelKey> = listOf(
    PanelKey.Special("esc", SpecialKey.ESCAPE),
    PanelKey.Special("tab", SpecialKey.TAB),
    PanelKey.Modifier("ctrl", StickyMod.CTRL),
    PanelKey.Modifier("alt", StickyMod.ALT),
    PanelKey.Special("←", SpecialKey.LEFT),
    PanelKey.Special("↑", SpecialKey.UP),
    PanelKey.Special("↓", SpecialKey.DOWN),
    PanelKey.Special("→", SpecialKey.RIGHT),
    PanelKey.Special("shift\ntab", SpecialKey.TAB, KeyMods(ctrl = false, alt = false, shift = true)),
    PanelKey.Text("-"),
    PanelKey.Text("|"),
    PanelKey.Text("/"),
    PanelKey.Text("~"),
    PanelKey.Text(":"),
)

/**
 * Extra-keys panel above the soft keyboard, laid out like the Termius iOS one.
 *
 * Compact: one strip — grid toggle, a sideways-scrolling row of the everyday
 * keys, paste and the keyboard toggle. Expanded: Customize/Password shortcuts,
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
    onKeyPressed: () -> Unit,
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
            Row(
                Modifier.fillMaxWidth().height(BarHeight).padding(ChromePadding),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(KeyGap),
            ) {
                IconKey(Icons.Filled.GridView, stringResource(R.string.more_keys), Modifier.width(44.dp), onClick = onToggleExpanded)
                LazyRow(
                    Modifier.weight(1f),
                    horizontalArrangement = Arrangement.spacedBy(KeyGap),
                ) {
                    stripKeys.forEach { key ->
                        item(key = KeyActions.encode(key)) {
                            TextKey(key, active(key), Modifier.widthIn(min = 44.dp), height = StripKeyHeight) { press(it) }
                        }
                    }
                }
                IconKey(Icons.Filled.ContentPaste, stringResource(R.string.paste_2), Modifier.width(44.dp)) { onKeyPressed(); onPaste() }
                IconKey(
                    if (imeShown) Icons.Filled.KeyboardHide else Icons.Filled.Keyboard,
                    stringResource(R.string.toggle_keyboard),
                    Modifier.width(44.dp),
                    onClick = onToggleIme,
                )
            }
            return@Column
        }
        Column(Modifier.fillMaxWidth().padding(top = 8.dp, start = 8.dp, end = 8.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(KeyGap)) {
                WideKey(Icons.Filled.Tune, stringResource(R.string.customize), Modifier.weight(1f)) { onKeyPressed(); onCustomize() }
                WideKey(Icons.Filled.Password, stringResource(R.string.password), Modifier.weight(1f)) { onKeyPressed(); onHiddenInput() }
            }
            Spacer(Modifier.height(KeyGap))
            Column(
                Modifier
                    .fillMaxWidth()
                    .heightIn(max = GridMaxHeight)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(KeyGap),
            ) {
                rows.forEach { row -> KeyRow(row, ::active, ::press) }
                Spacer(Modifier.height(2.dp))
            }
        }
        Row(
            Modifier.fillMaxWidth().height(BarHeight + 4.dp).padding(ChromePadding),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(KeyGap),
        ) {
            IconKey(Icons.Filled.GridView, stringResource(R.string.hide_keys), Modifier.width(52.dp), selected = true, onClick = onToggleExpanded)
            IconKey(Icons.Filled.DataObject, stringResource(R.string.snippets), Modifier.width(52.dp)) { onKeyPressed(); onSnippets() }
            IconKey(Icons.Filled.History, stringResource(R.string.history_and_themes), Modifier.width(52.dp)) { onKeyPressed(); onPanel() }
            IconKey(Icons.Filled.AutoAwesome, stringResource(R.string.ask_ai), Modifier.width(52.dp)) { onKeyPressed(); onAskAi() }
            Spacer(Modifier.weight(1f))
            Box(Modifier.width(1.dp).height(22.dp).background(MaterialTheme.colorScheme.outlineVariant))
            Spacer(Modifier.width(2.dp))
            IconKey(
                if (imeShown) Icons.Filled.KeyboardHide else Icons.Filled.Keyboard,
                stringResource(R.string.toggle_keyboard),
                Modifier.width(52.dp),
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
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(KeyGap)) {
        row.forEachIndexed { i, key ->
            if (row.size == 8 && i == 4) Spacer(Modifier.width(KeyGap * 2))
            TextKey(key, active(key), Modifier.weight(1f), height = GridKeyHeight) { press(it) }
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
    onPress: (PanelKey) -> Unit,
) {
    val twoLine = key.label.contains('\n')
    val single = when {
        twoLine -> 11.sp
        key.label.length <= 3 -> 14.sp
        key.label.length <= 4 -> 11.sp
        else -> 10.sp
    }
    Box(
        modifier
            .height(height)
            .clip(KeyShape)
            .background(keyContainer(active))
            .clickable { onPress(key) }
            .padding(horizontal = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            key.label,
            fontSize = single,
            lineHeight = if (twoLine) 12.sp else 16.sp,
            textAlign = TextAlign.Center,
            maxLines = if (twoLine) 2 else 1,
            softWrap = twoLine,
            fontWeight = if (active) FontWeight.SemiBold else FontWeight.Medium,
            color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
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
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurface)
    }
}

@Composable
private fun IconKey(
    icon: ImageVector,
    description: String,
    modifier: Modifier = Modifier,
    selected: Boolean = false,
    onClick: () -> Unit,
) {
    Box(
        modifier
            .height(StripKeyHeight)
            .clip(KeyShape)
            .background(if (selected) keyContainer(true) else Color.Transparent)
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            icon,
            contentDescription = description,
            tint = if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.size(22.dp),
        )
    }
}
