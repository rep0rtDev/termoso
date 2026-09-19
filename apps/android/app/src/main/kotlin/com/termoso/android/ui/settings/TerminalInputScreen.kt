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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.components.TermosoSwitch
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

    SubScreen(stringResource(R.string.keyboard_gestures), onBack) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
            item { SectionLabel(stringResource(R.string.key_panel_rows)) }
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
                        title = stringResource(R.string.add_row),
                        leading = { IconTile(Icons.Filled.Add) },
                        subtitle = if (groups.size >= MAX_GROUPS) stringResource(R.string.up_to_rows, MAX_GROUPS) else null,
                        modifier = Modifier.clickable(enabled = groups.size < MAX_GROUPS) {
                            editing = KeyGroup(id = KeyGroups.newGroupId(groups), name = str(R.string.custom), keys = emptyList(), enabled = true)
                        },
                    )
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.reset_to_defaults),
                        leading = { IconTile(Icons.Filled.RestartAlt) },
                        modifier = Modifier.clickable { confirmReset = true },
                    )
                }
            }

            item { SectionLabel(stringResource(R.string.volume_buttons)) }
            item {
                SectionCard {
                    ChevronRow(
                        title = stringResource(R.string.volume_up),
                        badge = InputAction.title(InputAction.parse(settings.volumeUpAction)),
                        modifier = Modifier.clickable { volumePick = true },
                    )
                    RowDivider()
                    ChevronRow(
                        title = stringResource(R.string.volume_down),
                        badge = InputAction.title(InputAction.parse(settings.volumeDownAction)),
                        modifier = Modifier.clickable { volumePick = false },
                    )
                    Text(
                        stringResource(R.string.bound_buttons_act_only_while_a_terminal_is),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }

            item { SectionLabel(stringResource(R.string.physical_keyboard)) }
            item {
                SectionCard {
                    ChevronRow(
                        title = stringResource(R.string.app_shortcuts),
                        subtitle = stringResource(R.string.modifier_for_tabs_text_size_paste_and_the),
                        badge = HardwareKeys.hotkeyModes.firstOrNull { it.first == settings.hardwareHotkeys }?.let { stringResource(it.second) } ?: settings.hardwareHotkeys,
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
                        title = stringResource(R.string.hide_key_panel_with_a_keyboard),
                        subtitle = stringResource(R.string.collapse_the_panel_to_one_line_when_a),
                        checked = settings.hidePanelWithKeyboard,
                        onCheckedChange = { v -> set { it.copy(hidePanelWithKeyboard = v) } },
                    )
                    RowDivider()
                    SwitchRow(
                        title = stringResource(R.string.autocomplete),
                        subtitle = stringResource(R.string.suggest_commands_options_paths_history_and_snippets_above),
                        checked = settings.autocomplete,
                        onCheckedChange = { v -> set { it.copy(autocomplete = v) } },
                    )
                }
            }

            item { SectionLabel(stringResource(R.string.gestures)) }
            item {
                SectionCard {
                    SwitchRow(
                        title = stringResource(R.string.pinch_to_change_text_size),
                        checked = settings.pinchZoom,
                        onCheckedChange = { v -> set { it.copy(pinchZoom = v) } },
                    )
                    RowDivider()
                    SwitchRow(
                        title = stringResource(R.string.swipe_for),
                        subtitle = stringResource(R.string.a_horizontal_one_finger_swipe_moves_the_cursor),
                        checked = settings.swipeArrows,
                        onCheckedChange = { v -> set { it.copy(swipeArrows = v) } },
                    )
                    RowDivider()
                    SwitchRow(
                        title = stringResource(R.string.two_finger_swipe_switches_sessions),
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
            title = if (up) stringResource(R.string.volume_up) else stringResource(R.string.volume_down),
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
        RadioDialog(stringResource(R.string.app_shortcuts), HardwareKeys.hotkeyModes.map { it.first to stringResource(it.second) }, settings.hardwareHotkeys, onPick = { set { s -> s.copy(hardwareHotkeys = it) } }) {
            hotkeyPick = false
        }
    }
    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text(stringResource(R.string.reset_key_panel)) },
            text = { Text(stringResource(R.string.your_custom_rows_are_replaced_by_the_built)) },
            confirmButton = {
                TextButton(onClick = { confirmReset = false; saveGroups(emptyList()) }) { Text(stringResource(R.string.reset)) }
            },
            dismissButton = { TextButton(onClick = { confirmReset = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private fun shortcutHelp(mode: String): String {
    val m = when (mode) {
        "ctrl" -> "Ctrl"
        "ctrl_shift" -> "Ctrl+Shift"
        else -> return str(R.string.every_key_combination_goes_to_the_shell)
    }
    return str(R.string.switch_session_t_new_n_clone_w_close, m, m, m, m, m)
}

@Composable
private fun GroupRow(group: KeyGroup, first: Boolean, last: Boolean, onToggle: (Boolean) -> Unit, onMove: (Int) -> Unit, onEdit: () -> Unit) {
    val preview = group.keys.mapNotNull(KeyActions::parse).joinToString("  ") { it.label }
    ListRow(
        title = group.name,
        subtitle = preview.ifEmpty { stringResource(R.string.no_keys_tap_to_add) },
        leading = { TermosoSwitch(checked = group.enabled, onCheckedChange = onToggle) },
        modifier = Modifier.clickable(onClick = onEdit),
    ) {
        IconButton(onClick = { onMove(-1) }, enabled = !first) { Icon(Icons.Filled.ArrowUpward, contentDescription = stringResource(R.string.move_up)) }
        IconButton(onClick = { onMove(1) }, enabled = !last) { Icon(Icons.Filled.ArrowDownward, contentDescription = stringResource(R.string.move_down)) }
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
        title = stringResource(R.string.edit_row),
        onBack = ::leave,
        actions = {
            IconButton(onClick = { confirmDelete = true }) { Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.delete_row)) }
        },
    ) { padding ->
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp)) {
            item {
                FormField(
                    value = name,
                    onChange = { name = it.take(MAX_NAME) },
                    label = stringResource(R.string.name),
                )
                Spacer(Modifier.height(8.dp))
            }
            item { SectionLabel(stringResource(R.string.keys_2, keys.size, MAX_KEYS)) }
            item {
                SectionCard {
                    if (keys.isEmpty()) {
                        Text(
                            stringResource(R.string.empty_rows_are_hidden_in_the_terminal),
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
                            subtitle = parsed?.let(KeyActions::describe) ?: stringResource(R.string.unknown_action_skipped, def.action),
                            modifier = Modifier.clickable(enabled = parsed != null) { relabel = i },
                        ) {
                            IconButton(onClick = { onChange(group.copy(keys = keys.moved(i, i - 1))) }, enabled = i > 0) {
                                Icon(Icons.Filled.ArrowUpward, contentDescription = stringResource(R.string.move_left))
                            }
                            IconButton(onClick = { onChange(group.copy(keys = keys.moved(i, i + 1))) }, enabled = i < keys.lastIndex) {
                                Icon(Icons.Filled.ArrowDownward, contentDescription = stringResource(R.string.move_right))
                            }
                            IconButton(onClick = { onChange(group.copy(keys = keys.filterIndexed { j, _ -> j != i })) }) {
                                Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.remove_2))
                            }
                        }
                    }
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.add_key),
                        leading = { IconTile(Icons.Filled.Keyboard) },
                        subtitle = if (keys.size >= MAX_KEYS) stringResource(R.string.up_to_keys_per_row, MAX_KEYS) else null,
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
            title = { Text(stringResource(R.string.add_key)) },
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
            confirmButton = { TextButton(onClick = { palette = false }) { Text(stringResource(R.string.close)) } },
        )
    }
    relabel?.let { index ->
        val def = keys.getOrNull(index) ?: run { relabel = null; return@let }
        var label by remember(index) { mutableStateOf(def.label) }
        AlertDialog(
            onDismissRequest = { relabel = null },
            title = { Text(stringResource(R.string.key_label)) },
            text = {
                Column {
                    Text(KeyActions.parse(def)?.let(KeyActions::describe) ?: def.action, style = MaterialTheme.typography.bodyMedium)
                    Spacer(Modifier.height(12.dp))
                    FormField(value = label, onChange = { label = it.take(MAX_LABEL) }, label = stringResource(R.string.shown_on_the_key))
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val shown = label.ifBlank { KeyActions.parse(def)?.label ?: def.label }
                        onChange(group.copy(keys = keys.mapIndexed { j, k -> if (j == index) PanelKeyDef(shown, k.action) else k }))
                        relabel = null
                    },
                ) { Text(stringResource(R.string.save)) }
            },
            dismissButton = { TextButton(onClick = { relabel = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text(stringResource(R.string.delete_2, group.name)) },
            text = { Text(stringResource(R.string.the_row_and_its_keys_are_removed_from)) },
            confirmButton = { TextButton(onClick = { confirmDelete = false; onDelete() }) { Text(stringResource(R.string.delete)) } },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

private fun <T> List<T>.moved(from: Int, to: Int): List<T> {
    if (from !in indices || to !in indices || from == to) return this
    val m = toMutableList()
    m.add(to, m.removeAt(from))
    return m
}
