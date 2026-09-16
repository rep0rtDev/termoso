package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardTab
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.DataObject
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.KeyboardHide
import androidx.compose.material.icons.filled.MoreHoriz
import androidx.compose.material.icons.filled.Password
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.termoso.core.KeyMods
import com.termoso.core.SpecialKey

/** Something the key panel can send. */
sealed interface PanelKey {
    val label: String

    data class Special(override val label: String, val key: SpecialKey, val shift: Boolean = false) : PanelKey
    data class Text(override val label: String, val text: String = label) : PanelKey
    data class Modifier(override val label: String, val which: StickyMod) : PanelKey
}

enum class StickyMod { CTRL, ALT, SHIFT }

private val extraRows: List<List<PanelKey>> = listOf(
    listOf(
        PanelKey.Special("←", SpecialKey.LEFT), PanelKey.Special("↑", SpecialKey.UP),
        PanelKey.Special("↓", SpecialKey.DOWN), PanelKey.Special("→", SpecialKey.RIGHT),
        PanelKey.Modifier("Alt", StickyMod.ALT), PanelKey.Special("Tab", SpecialKey.TAB),
        PanelKey.Special("Insert", SpecialKey.INSERT), PanelKey.Special("Delete", SpecialKey.DELETE),
    ),
    listOf(
        PanelKey.Special("Home", SpecialKey.HOME), PanelKey.Special("Pg Up", SpecialKey.PAGE_UP),
        PanelKey.Special("Pg Dn", SpecialKey.PAGE_DOWN), PanelKey.Special("End", SpecialKey.END),
        PanelKey.Text("|"), PanelKey.Text("\\"), PanelKey.Text("?"), PanelKey.Text("-"),
    ),
    listOf(
        PanelKey.Text("/"), PanelKey.Text(":"), PanelKey.Text(";"), PanelKey.Text("!"),
        PanelKey.Text("~"), PanelKey.Text("@"), PanelKey.Text("$"), PanelKey.Text("*"),
    ),
    listOf(
        PanelKey.Text("^"), PanelKey.Text("%"), PanelKey.Text("="), PanelKey.Text("`"),
        PanelKey.Text("<"), PanelKey.Text(">"), PanelKey.Text("("), PanelKey.Text(")"),
    ),
    listOf(
        PanelKey.Text("{"), PanelKey.Text("}"), PanelKey.Text("["), PanelKey.Text("]"),
        PanelKey.Special("F1", SpecialKey.F1), PanelKey.Special("F2", SpecialKey.F2),
        PanelKey.Special("F3", SpecialKey.F3), PanelKey.Special("F4", SpecialKey.F4),
    ),
    listOf(
        PanelKey.Special("F5", SpecialKey.F5), PanelKey.Special("F6", SpecialKey.F6),
        PanelKey.Special("F7", SpecialKey.F7), PanelKey.Special("F8", SpecialKey.F8),
        PanelKey.Special("F9", SpecialKey.F9), PanelKey.Special("F10", SpecialKey.F10),
        PanelKey.Special("F11", SpecialKey.F11), PanelKey.Special("F12", SpecialKey.F12),
    ),
)

/**
 * Extra-keys bar above the soft keyboard. The first row is always there; the
 * ⋯ toggle unfolds arrows, navigation, symbols and F-keys. Ctrl/Alt/Shift are
 * sticky: they apply to the next key or character, then release.
 */
@Composable
fun KeyPanel(
    controller: TerminalController,
    expanded: Boolean,
    imeShown: Boolean,
    onToggleExpanded: () -> Unit,
    onToggleIme: () -> Unit,
    onHiddenInput: () -> Unit,
    onSnippets: () -> Unit,
    onPanel: () -> Unit,
    onPaste: () -> Unit,
    onKeyPressed: () -> Unit,
    modifier: Modifier = Modifier,
) {
    fun press(key: PanelKey) {
        onKeyPressed()
        when (key) {
            is PanelKey.Special -> controller.sendKey(
                key.key,
                KeyMods(ctrl = false, alt = false, shift = key.shift),
            )
            is PanelKey.Text -> controller.sendText(key.text)
            is PanelKey.Modifier -> when (key.which) {
                StickyMod.CTRL -> controller.ctrl = !controller.ctrl
                StickyMod.ALT -> controller.alt = !controller.alt
                StickyMod.SHIFT -> controller.shift = !controller.shift
            }
        }
    }

    fun active(key: PanelKey): Boolean = key is PanelKey.Modifier && when (key.which) {
        StickyMod.CTRL -> controller.ctrl
        StickyMod.ALT -> controller.alt
        StickyMod.SHIFT -> controller.shift
    }

    Column(modifier.background(MaterialTheme.colorScheme.surfaceContainer)) {
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
        Row(Modifier.fillMaxWidth().height(44.dp), verticalAlignment = Alignment.CenterVertically) {
            IconKey(Icons.Filled.Password, "Hidden input", Modifier.weight(1f)) { onKeyPressed(); onHiddenInput() }
            IconKey(Icons.Filled.DataObject, "Snippets", Modifier.weight(1f)) { onKeyPressed(); onSnippets() }
            IconKey(Icons.Filled.History, "History and themes", Modifier.weight(1f)) { onKeyPressed(); onPanel() }
            IconKey(Icons.Filled.ContentPaste, "Paste", Modifier.weight(1f)) { onKeyPressed(); onPaste() }
            TextKey(
                PanelKey.Special("shift\ntab", SpecialKey.TAB, shift = true),
                active = false,
                modifier = Modifier.weight(1f),
                small = true,
            ) { press(it) }
            TextKey(PanelKey.Modifier("Ctrl", StickyMod.CTRL), active(PanelKey.Modifier("Ctrl", StickyMod.CTRL)), Modifier.weight(1f)) { press(it) }
            TextKey(PanelKey.Special("Esc", SpecialKey.ESCAPE), active = false, Modifier.weight(1f)) { press(it) }
            TextKey(PanelKey.Special("Tab", SpecialKey.TAB), active = false, Modifier.weight(1f)) { press(it) }
            Box(Modifier.width(1.dp).height(28.dp).background(MaterialTheme.colorScheme.outlineVariant))
            IconKey(Icons.Filled.MoreHoriz, "More keys", Modifier.weight(1f), tint = if (expanded) MaterialTheme.colorScheme.primary else null, onClick = onToggleExpanded)
            IconKey(
                if (imeShown) Icons.Filled.KeyboardHide else Icons.Filled.Keyboard,
                "Toggle keyboard",
                Modifier.weight(1f),
                onClick = onToggleIme,
            )
        }
        if (expanded) {
            extraRows.forEach { row ->
                Row(Modifier.fillMaxWidth().height(40.dp), verticalAlignment = Alignment.CenterVertically) {
                    row.forEach { key -> TextKey(key, active(key), Modifier.weight(1f)) { press(it) } }
                }
            }
        }
    }
}

@Composable
private fun TextKey(
    key: PanelKey,
    active: Boolean,
    modifier: Modifier = Modifier,
    small: Boolean = false,
    onPress: (PanelKey) -> Unit,
) {
    Box(
        modifier.fillMaxWidth().height(if (small) 44.dp else 40.dp).clickable { onPress(key) },
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                key.label,
                fontSize = if (small) 12.sp else 14.sp,
                lineHeight = if (small) 13.sp else 16.sp,
                textAlign = TextAlign.Center,
                fontWeight = if (active) FontWeight.Bold else FontWeight.Medium,
                color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
            if (key is PanelKey.Modifier) {
                Box(
                    Modifier
                        .padding(top = 2.dp)
                        .width(20.dp)
                        .height(2.dp)
                        .background(if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant),
                )
            }
        }
    }
}

@Composable
private fun IconKey(
    icon: ImageVector,
    description: String,
    modifier: Modifier = Modifier,
    tint: androidx.compose.ui.graphics.Color? = null,
    onClick: () -> Unit,
) {
    Box(modifier.fillMaxWidth().height(40.dp).clickable(onClick = onClick), contentAlignment = Alignment.Center) {
        Icon(icon, contentDescription = description, tint = tint ?: MaterialTheme.colorScheme.onSurface)
    }
}
