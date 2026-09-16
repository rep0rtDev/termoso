package com.termoso.android.ui.settings

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.RestartAlt
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.HardwareKeys
import com.termoso.android.ui.terminal.InputAction
import com.termoso.android.ui.terminal.KeyActions
import com.termoso.android.ui.terminal.KeyGroups
import com.termoso.core.KeyGroup
import com.termoso.core.MobileSettings
import com.termoso.core.PanelKeyDef
import kotlinx.coroutines.launch

/** Mirrors `MAX_KEY_GROUPS` / `MAX_KEYS_PER_GROUP` in the Rust settings sanitiser. */
private const val MAX_GROUPS = 24
private const val MAX_KEYS = 16
private const val MAX_LABEL = 12
private const val MAX_NAME = 32

/**
 * Settings → Keyboard & gestures: the extra-key rows of the terminal panel,
 * what the volume buttons do, physical-keyboard behaviour and touch gestures.
 * Everything is stored in [MobileSettings] and therefore synced with the vault.
 */
@Composable
fun TerminalInputScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val groups = remember(settings.keyGroups) { KeyGroups.editable(settings.keyGroups) }

    fun set(transform: (MobileSettings) -> MobileSettings) {
        scope.launch { shell.repo.updateSettings(transform) }
    }
    fun saveGroups(list: List<KeyGroup>) = set { it.copy(keyGroups = list) }

    // The row being edited lives here as a draft: Rust drops rows without keys
    // on save, so a freshly added row would vanish if the editor read it back
    // from the settings flow.
    var editing by remember { mutableStateOf<KeyGroup?>(null) }
    fun upsert(g: KeyGroup): List<KeyGroup> =
        if (groups.any { it.id == g.id }) groups.map { if (it.id == g.id) g else it } else groups + g

    editing?.let { draft ->
        KeyGroupEditor(
            group = draft,
            onChange = { g ->
                editing = g
                saveGroups(upsert(g))
            },
            onDelete = {
                saveGroups(groups.filter { it.id != draft.id })
                editing = null
            },
            onBack = { g ->
                saveGroups(upsert(g))
                editing = null
            },
        )
        return
    }

    var volumePick by remember { mutableStateOf<Boolean?>(null) }
    var hotkeyPick by remember { mutableStateOf(false) }
    var confirmReset by remember { mutableStateOf(false) }

    SubScreen("Keyboard & gestures", onBack) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
            item { SectionLabel("Key panel rows") }
            item {
                SectionCard {
                    groups.forEachIndexed { i, g ->
                        if (i > 0) RowDivider()
                        GroupRow(
                            group = g,
                            first = i == 0,
                            last = i == groups.lastIndex,
                            onToggle = { on -> saveGroups(groups.map { if (it.id == g.id) it.copy(enabled = on) else it }) },
                            onMove = { delta -> saveGroups(groups.moved(i, i + delta)) },
                            onEdit = { editing = g },
                        )
                    }
                    RowDivider()
                    ListRow(
                        title = "Add row",
                        leading = { IconTile(Icons.Filled.Add) },
                        subtitle = if (groups.size >= MAX_GROUPS) "Up to $MAX_GROUPS rows" else null,
                        modifier = Modifier.clickable(enabled = groups.size < MAX_GROUPS) {
                            editing = KeyGroup(id = KeyGroups.newGroupId(groups), name = "Custom", keys = emptyList(), enabled = true)
                        },
                    )
                    RowDivider()
                    ListRow(
                        title = "Reset to defaults",
                        leading = { IconTile(Icons.Filled.RestartAlt) },
                        modifier = Modifier.clickable { confirmReset = true },
                    )
                }
            }

            item { SectionLabel("Volume buttons") }
            item {
                SectionCard {
                    ChevronRow(
                        title = "Volume up",
                        badge = InputAction.title(InputAction.parse(settings.volumeUpAction)),
                        modifier = Modifier.clickable { volumePick = true },
                    )
                    RowDivider()
                    ChevronRow(
                        title = "Volume down",
                        badge = InputAction.title(InputAction.parse(settings.volumeDownAction)),
                        modifier = Modifier.clickable { volumePick = false },
                    )
                    Text(
                        "Bound buttons act only while a terminal is on screen; elsewhere they change the volume as usual.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }

            item { SectionLabel("Physical keyboard") }
            item {
                SectionCard {
                    ChevronRow(
                        title = "App shortcuts",
                        subtitle = "Modifier for tabs, text size, paste and the key panel",
                        badge = HardwareKeys.hotkeyModes.firstOrNull { it.first == settings.hardwareHotkeys }?.second ?: settings.hardwareHotkeys,
                        modifier = Modifier.clickable { hotkeyPick = true },
                    )
                    Text(
                        shortcutHelp(settings.hardwareHotkeys),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                    RowDivider()
                    SwitchRow(
                        title = "Hide key panel with a keyboard",
                        subtitle = "Collapse the panel to one line when a physical keyboard is attached",
                        checked = settings.hidePanelWithKeyboard,
                        onCheckedChange = { v -> set { it.copy(hidePanelWithKeyboard = v) } },
                    )
                }
            }

            item { SectionLabel("Gestures") }
            item {
                SectionCard {
                    SwitchRow(
                        title = "Pinch to change text size",
                        checked = settings.pinchZoom,
                        onCheckedChange = { v -> set { it.copy(pinchZoom = v) } },
                    )
                    RowDivider()
                    SwitchRow(
                        title = "Swipe for ← →",
                        subtitle = "A horizontal one-finger swipe moves the cursor",
                        checked = settings.swipeArrows,
                        onCheckedChange = { v -> set { it.copy(swipeArrows = v) } },
                    )
                    RowDivider()
                    SwitchRow(
                        title = "Two-finger swipe switches sessions",
                        checked = settings.swipeSessions,
                        onCheckedChange = { v -> set { it.copy(swipeSessions = v) } },
                    )
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }

    volumePick?.let { up ->
        val current = InputAction.parse(if (up) settings.volumeUpAction else settings.volumeDownAction)
        RadioDialog(
            title = if (up) "Volume up" else "Volume down",
            options = InputAction.choices.map { it to InputAction.title(it) },
            selected = current,
            onPick = { a ->
                val v = InputAction.encode(a)
                set { if (up) it.copy(volumeUpAction = v) else it.copy(volumeDownAction = v) }
            },
            onDismiss = { volumePick = null },
        )
    }
    if (hotkeyPick) {
        RadioDialog("App shortcuts", HardwareKeys.hotkeyModes, settings.hardwareHotkeys, onPick = { set { s -> s.copy(hardwareHotkeys = it) } }) {
            hotkeyPick = false
        }
    }
    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text("Reset key panel?") },
            text = { Text("Your custom rows are replaced by the built-in layout.") },
            confirmButton = {
                TextButton(onClick = { confirmReset = false; saveGroups(emptyList()) }) { Text("Reset") }
            },
            dismissButton = { TextButton(onClick = { confirmReset = false }) { Text("Cancel") } },
        )
    }
}

private fun shortcutHelp(mode: String): String {
    val m = when (mode) {
        "ctrl" -> "Ctrl"
        "ctrl_shift" -> "Ctrl+Shift"
        else -> return "Every key combination goes to the shell."
    }
    return "$m+← / → switch session · $m+T new · $m+N clone · $m+W close · " +
        "$m+= / − / 0 text size · Ctrl+Shift+V paste · Ctrl+Shift+K key panel"
}

@Composable
private fun GroupRow(group: KeyGroup, first: Boolean, last: Boolean, onToggle: (Boolean) -> Unit, onMove: (Int) -> Unit, onEdit: () -> Unit) {
    val preview = group.keys.mapNotNull(KeyActions::parse).joinToString("  ") { it.label }
    ListRow(
        title = group.name,
        subtitle = preview.ifEmpty { "No keys — tap to add" },
        leading = { Switch(checked = group.enabled, onCheckedChange = onToggle) },
        modifier = Modifier.clickable(onClick = onEdit),
    ) {
        IconButton(onClick = { onMove(-1) }, enabled = !first) { Icon(Icons.Filled.ArrowUpward, contentDescription = "Move up") }
        IconButton(onClick = { onMove(1) }, enabled = !last) { Icon(Icons.Filled.ArrowDownward, contentDescription = "Move down") }
    }
}

/**
 * One row of the key panel: name, ordered keys, add from the palette, relabel,
 * remove. Key changes are saved at once; the name is committed on leaving so
 * typing does not round-trip through the encrypted store per character.
 */
@Composable
private fun KeyGroupEditor(group: KeyGroup, onChange: (KeyGroup) -> Unit, onDelete: () -> Unit, onBack: (KeyGroup) -> Unit) {
    var name by remember(group.id) { mutableStateOf(group.name) }
    fun leave() = onBack(group.copy(name = name.trim().ifBlank { group.name }))
    BackHandler(onBack = ::leave)
    var palette by remember { mutableStateOf(false) }
    var relabel by remember { mutableStateOf<Int?>(null) }
    var confirmDelete by remember { mutableStateOf(false) }
    val keys = group.keys

    SubScreen(
        title = "Edit row",
        onBack = ::leave,
        actions = {
            IconButton(onClick = { confirmDelete = true }) { Icon(Icons.Filled.Delete, contentDescription = "Delete row") }
        },
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
            item {
                FormField(
                    value = name,
                    onChange = { name = it.take(MAX_NAME) },
                    label = "Name",
                )
                Spacer(Modifier.height(8.dp))
            }
            item { SectionLabel("Keys · ${keys.size} / $MAX_KEYS") }
            item {
                SectionCard {
                    if (keys.isEmpty()) {
                        Text(
                            "Empty rows are hidden in the terminal.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(16.dp),
                        )
                    }
                    keys.forEachIndexed { i, def ->
                        if (i > 0) RowDivider()
                        val parsed = KeyActions.parse(def)
                        ListRow(
                            title = def.label.ifBlank { parsed?.label ?: def.action },
                            subtitle = parsed?.let(KeyActions::describe) ?: "Unknown action (${def.action}) — skipped",
                            modifier = Modifier.clickable(enabled = parsed != null) { relabel = i },
                        ) {
                            IconButton(onClick = { onChange(group.copy(keys = keys.moved(i, i - 1))) }, enabled = i > 0) {
                                Icon(Icons.Filled.ArrowUpward, contentDescription = "Move left")
                            }
                            IconButton(onClick = { onChange(group.copy(keys = keys.moved(i, i + 1))) }, enabled = i < keys.lastIndex) {
                                Icon(Icons.Filled.ArrowDownward, contentDescription = "Move right")
                            }
                            IconButton(onClick = { onChange(group.copy(keys = keys.filterIndexed { j, _ -> j != i })) }) {
                                Icon(Icons.Filled.Close, contentDescription = "Remove")
                            }
                        }
                    }
                    RowDivider()
                    ListRow(
                        title = "Add key",
                        leading = { IconTile(Icons.Filled.Keyboard) },
                        subtitle = if (keys.size >= MAX_KEYS) "Up to $MAX_KEYS keys per row" else null,
                        modifier = Modifier.clickable(enabled = keys.size < MAX_KEYS) { palette = true },
                    )
                }
            }
            item { Spacer(Modifier.height(24.dp)) }
        }
    }

    if (palette) {
        AlertDialog(
            onDismissRequest = { palette = false },
            title = { Text("Add key") },
            text = {
                LazyVerticalGrid(
                    columns = GridCells.Adaptive(72.dp),
                    modifier = Modifier.heightIn(max = 420.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    items(KeyGroups.palette) { key ->
                        OutlinedButton(
                            onClick = {
                                onChange(group.copy(keys = keys + KeyActions.toDef(key)))
                                palette = false
                            },
                            contentPadding = PaddingValues(horizontal = 4.dp, vertical = 8.dp),
                        ) {
                            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                                Text(key.label, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                Text(
                                    KeyActions.describe(key),
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { palette = false }) { Text("Close") } },
        )
    }
    relabel?.let { index ->
        val def = keys.getOrNull(index) ?: run { relabel = null; return@let }
        var label by remember(index) { mutableStateOf(def.label) }
        AlertDialog(
            onDismissRequest = { relabel = null },
            title = { Text("Key label") },
            text = {
                Column {
                    Text(KeyActions.parse(def)?.let(KeyActions::describe) ?: def.action, style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.height(12.dp))
                    FormField(value = label, onChange = { label = it.take(MAX_LABEL) }, label = "Shown on the key")
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val shown = label.ifBlank { KeyActions.parse(def)?.label ?: def.label }
                        onChange(group.copy(keys = keys.mapIndexed { j, k -> if (j == index) PanelKeyDef(shown, k.action) else k }))
                        relabel = null
                    },
                ) { Text("Save") }
            },
            dismissButton = { TextButton(onClick = { relabel = null }) { Text("Cancel") } },
        )
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete “${group.name}”?") },
            text = { Text("The row and its keys are removed from the panel.") },
            confirmButton = { TextButton(onClick = { confirmDelete = false; onDelete() }) { Text("Delete") } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Cancel") } },
        )
    }
}

private fun <T> List<T>.moved(from: Int, to: Int): List<T> {
    if (from !in indices || to !in indices || from == to) return this
    val m = toMutableList()
    m.add(to, m.removeAt(from))
    return m
}
