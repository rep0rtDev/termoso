package com.termoso.android.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.str
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.components.groupRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.terminalFonts
import com.termoso.core.MobileSettings
import com.termoso.core.TerminalPalette
import com.termoso.core.TerminalTheme
import com.termoso.core.terminalTheme
import com.termoso.core.terminalThemes
import kotlinx.coroutines.launch

private val cursorStyles = listOf("block" to R.string.block, "underline" to R.string.underline, "beam" to R.string.bar)

/** Colour scheme, font family/size and cursor for the terminal; changes apply to open sessions at once. */
@Composable
fun TerminalAppearanceScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val themes = remember { terminalThemes() }
    val darkThemes = remember(themes) { themes.filter { it.dark } }
    val lightThemes = remember(themes) { themes.filter { !it.dark } }
    val current = remember(settings.terminalTheme) { terminalTheme(settings.terminalTheme) ?: themes.first() }

    fun set(transform: (MobileSettings) -> MobileSettings) {
        scope.launch { shell.repo.updateSettings(transform) }
    }
    fun pickTheme(theme: TerminalTheme) {
        scope.launch {
            shell.repo.updateSettings { it.copy(terminalTheme = theme.id) }
            shell.sessions.applyPalette(theme.palette)
        }
    }

    SubScreen(stringResource(R.string.terminal_appearance), onBack) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
            item {
                TerminalPreview(current.palette, settings.terminalFontFamily, settings.terminalFontSize.toInt(), settings.cursorStyle)
            }

            item { SectionLabel(stringResource(R.string.font)) }
            item {
                SectionCard {
                    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(stringResource(R.string.size), Modifier.width(56.dp))
                        IconButton(onClick = { set { it.copy(terminalFontSize = (it.terminalFontSize - 1u).coerceAtLeast(6u)) } }) {
                            Icon(Icons.Filled.Remove, contentDescription = stringResource(R.string.smaller))
                        }
                        Slider(
                            value = settings.terminalFontSize.toFloat(),
                            onValueChange = { v -> set { it.copy(terminalFontSize = v.toInt().toUInt()) } },
                            valueRange = 6f..40f,
                            steps = 33,
                            modifier = Modifier.weight(1f),
                        )
                        IconButton(onClick = { set { it.copy(terminalFontSize = (it.terminalFontSize + 1u).coerceAtMost(40u)) } }) {
                            Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.larger))
                        }
                        Text("${settings.terminalFontSize}", Modifier.width(28.dp), style = MaterialTheme.typography.bodyMedium)
                    }
                    terminalFonts.forEach { font ->
                        RowDivider()
                        val selected = font.name.equals(settings.terminalFontFamily, ignoreCase = true)
                        Row(
                            Modifier.fillMaxWidth().clickable { set { it.copy(terminalFontFamily = font.name) } }.padding(horizontal = 16.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(selected = selected, onClick = null)
                            Spacer(Modifier.width(12.dp))
                            Column {
                                Text(font.name, style = MaterialTheme.typography.bodyLarge)
                                Text(
                                    "ssh user@host -p 2222 · 0O1lI {}[]",
                                    fontFamily = composeFamily(font.name),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }

            item { SectionLabel(stringResource(R.string.cursor)) }
            item {
                SectionCard {
                    SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth().padding(16.dp)) {
                        cursorStyles.forEachIndexed { i, (id, label) ->
                            SegmentedButton(
                                selected = settings.cursorStyle == id,
                                onClick = { set { it.copy(cursorStyle = id) } },
                                shape = SegmentedButtonDefaults.itemShape(i, cursorStyles.size),
                            ) { Text(stringResource(label)) }
                        }
                    }
                    RowDivider()
                    SwitchRow(title = stringResource(R.string.blink), checked = settings.cursorBlink, onCheckedChange = { v -> set { it.copy(cursorBlink = v) } })
                }
            }

            item { SectionLabel(stringResource(R.string.color_scheme)) }
            themeGroup(str(R.string.dark), darkThemes, current.id, top = true, bottom = false, onPick = ::pickTheme)
            themeGroup(str(R.string.light), lightThemes, current.id, top = false, bottom = true, onPick = ::pickTheme)
            item { Spacer(Modifier.height(24.dp)) }
        }
    }
}

/**
 * Header + one lazy item per theme, so the gallery (dozens of swatches) is
 * composed row by row as it scrolls into view instead of all at once.
 */
private fun LazyListScope.themeGroup(
    title: String,
    items: List<TerminalTheme>,
    selectedId: String,
    top: Boolean,
    bottom: Boolean,
    onPick: (TerminalTheme) -> Unit,
) {
    item(key = "theme-group-$title", contentType = "theme-header") {
        Column(Modifier.groupRow(top = top, bottom = false)) {
            if (!top) RowDivider()
            Text(
                "$title · ${items.size}",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp),
            )
        }
    }
    itemsIndexed(items, key = { _, t -> "theme-${t.id}" }, contentType = { _, _ -> "theme" }) { i, theme ->
        Column(Modifier.groupRow(top = false, bottom = bottom && i == items.lastIndex)) {
            if (i > 0) RowDivider()
            ListRow(
                title = theme.name,
                leading = { PaletteSwatch(theme.palette) },
                modifier = Modifier.clickable { onPick(theme) },
                trailing = {
                    if (theme.id == selectedId) Icon(Icons.Filled.Check, contentDescription = str(R.string.selected), tint = MaterialTheme.colorScheme.primary)
                },
            )
        }
    }
}

/** Background tile with the eight normal ANSI colours, like the desktop theme gallery. */
@Composable
fun PaletteSwatch(p: TerminalPalette) {
    Row(
        Modifier.size(width = 56.dp, height = 36.dp).clip(RoundedCornerShape(8.dp)).background(rgb(p.background)).padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        p.ansi.take(8).forEachIndexed { i, c ->
            if (i == 0) return@forEachIndexed
            Box(Modifier.weight(1f).height(16.dp).clip(RoundedCornerShape(2.dp)).background(rgb(c)))
        }
    }
}

@Composable
private fun TerminalPreview(p: TerminalPalette, family: String, sizeSp: Int, cursor: String) {
    val font = composeFamily(family)
    val fg = rgb(p.foreground)
    val sz = sizeSp.coerceIn(8, 22).sp
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(rgb(p.background)).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Row {
            Text("user", color = rgb(p.ansi[2]), fontFamily = font, fontSize = sz, fontWeight = FontWeight.Bold)
            Text("@", color = fg, fontFamily = font, fontSize = sz)
            Text("termoso", color = rgb(p.ansi[4]), fontFamily = font, fontSize = sz, fontWeight = FontWeight.Bold)
            Text(stringResource(R.string.ls_la), color = fg, fontFamily = font, fontSize = sz)
        }
        Row {
            Text("drwxr-xr-x ", color = rgb(p.ansi[4]), fontFamily = font, fontSize = sz)
            Text("bin  ", color = rgb(p.ansi[1]), fontFamily = font, fontSize = sz)
            Text("docs  ", color = rgb(p.ansi[3]), fontFamily = font, fontSize = sz)
            Text("src  ", color = rgb(p.ansi[5]), fontFamily = font, fontSize = sz)
            Text("main.rs", color = rgb(p.ansi[6]), fontFamily = font, fontSize = sz)
        }
        Row(verticalAlignment = Alignment.Bottom) {
            Text("user@termoso:~$ ", color = fg, fontFamily = font, fontSize = sz)
            val cur = rgb(p.cursor)
            when (cursor) {
                "underline" -> Box(Modifier.width((sizeSp * 0.6f).dp).height(2.dp).background(cur))
                "beam" -> Box(Modifier.width(2.dp).height((sizeSp * 1.2f).dp).background(cur))
                else -> Box(Modifier.width((sizeSp * 0.6f).dp).height((sizeSp * 1.2f).dp).background(cur))
            }
        }
        Spacer(Modifier.height(4.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            p.ansi.take(8).forEach { c -> Box(Modifier.weight(1f).height(10.dp).clip(RoundedCornerShape(2.dp)).background(rgb(c))) }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(3.dp)) {
            p.ansi.drop(8).take(8).forEach { c -> Box(Modifier.weight(1f).height(10.dp).clip(RoundedCornerShape(2.dp)).background(rgb(c))) }
        }
    }
}

internal fun rgb(v: UInt) = Color(v.toInt() or (0xFF shl 24))

/** Compose font family for a registry name; falls back to the platform monospace. */
fun composeFamily(name: String): FontFamily {
    val font = terminalFonts.firstOrNull { it.name.equals(name.trim(), ignoreCase = true) }
    val res = font?.regular ?: return FontFamily.Monospace
    return FontFamily(
        listOfNotNull(
            Font(res, FontWeight.Normal),
            font.bold?.let { Font(it, FontWeight.Bold) },
        ),
    )
}
