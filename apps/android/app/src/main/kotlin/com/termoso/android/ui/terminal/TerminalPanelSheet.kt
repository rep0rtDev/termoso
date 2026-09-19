package com.termoso.android.ui.terminal

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.settings.PaletteSwatch
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.CommandHistoryItem
import com.termoso.core.SnippetDraft
import com.termoso.core.TerminalTheme
import com.termoso.core.VaultAccess
import com.termoso.core.terminalTheme
import com.termoso.core.terminalThemes
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.launch

/**
 * The terminal's side panel as a bottom sheet (Termius keeps it at the right
 * on desktop): commands typed in any terminal, taken from the vault's
 * encrypted history, and the theme gallery applied to this terminal only.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TerminalPanelSheet(
    shell: ShellViewModel,
    session: TerminalSession,
    controller: TerminalController,
    onClose: () -> Unit,
) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    ModalBottomSheet(onDismissRequest = onClose) {
        TabRow(selectedTabIndex = tab) {
            Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text(stringResource(R.string.history)) })
            Tab(selected = tab == 1, onClick = { tab = 1 }, text = { Text(stringResource(R.string.themes)) })
        }
        when (tab) {
            0 -> HistoryTab(shell, session, controller, onClose)
            else -> ThemesTab(shell, session)
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryTab(shell: ShellViewModel, session: TerminalSession, controller: TerminalController, onClose: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    var items by remember { mutableStateOf<List<CommandHistoryItem>?>(null) }
    var query by remember { mutableStateOf("") }
    var thisHost by rememberSaveable { mutableStateOf(session.hostId != null) }
    var menuFor by remember { mutableStateOf<CommandHistoryItem?>(null) }
    var saveAs by remember { mutableStateOf<CommandHistoryItem?>(null) }
    var confirmClear by remember { mutableStateOf(false) }

    LaunchedEffect(revision) {
        runCatching { shell.repo.read { commandHistory(500u) } }
            .onSuccess { items = it }
            .onFailure { items = emptyList(); shell.notify(it.userMessage()) }
    }

    val shown = items.orEmpty().filter { h ->
        (!thisHost || session.hostId == null || h.hostId == session.hostId) &&
            (query.isBlank() || h.command.contains(query, ignoreCase = true))
    }
    val canWrite by session.canWrite.collectAsStateWithLifecycle()
    val canType = !session.isView || canWrite

    Column(Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            if (session.hostId != null) {
                FilterChip(selected = thisHost, onClick = { thisHost = true }, label = { Text(stringResource(R.string.this_host)) })
                Spacer(Modifier.width(8.dp))
                FilterChip(selected = !thisHost, onClick = { thisHost = false }, label = { Text(stringResource(R.string.all_hosts)) })
            } else {
                SectionLabel(stringResource(R.string.commands_typed_in_your_terminals))
            }
            Spacer(Modifier.weight(1f))
            if (!items.isNullOrEmpty()) TextButton(onClick = { confirmClear = true }) { Text(stringResource(R.string.clear)) }
        }
        if (items.orEmpty().size > 8) FormField(query, { query = it }, stringResource(R.string.search))
    }
    when {
        items == null -> Spacer(Modifier.height(120.dp))
        shown.isEmpty() -> {
            EmptyState(
                title = if (query.isBlank()) stringResource(R.string.no_commands_yet) else stringResource(R.string.nothing_matches),
                hint = if (query.isBlank()) stringResource(R.string.commands_you_run_are_saved_here_encrypted_with) else stringResource(R.string.try_another_part_of_the_command),
                icon = Icons.Filled.Terminal,
                modifier = Modifier.padding(16.dp),
            )
            Spacer(Modifier.height(24.dp))
        }
        else -> LazyColumn(contentPadding = PaddingValues(bottom = 32.dp)) {
            items(shown, key = { it.id }) { h ->
                ListRow(
                    title = h.command,
                    subtitle = DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(h.at)),
                    leading = { IconTile(Icons.Filled.Terminal) },
                    modifier = Modifier.combinedClickable(
                        onClick = {
                            if (!canType) {
                                shell.notify(str(R.string.you_can_only_watch_this_terminal))
                                return@combinedClickable
                            }
                            controller.runCommand(h.command)
                            onClose()
                        },
                        onLongClick = { menuFor = h },
                    ),
                    trailing = {
                        IconButton(onClick = { menuFor = h }) { Icon(Icons.Filled.MoreVert, contentDescription = stringResource(R.string.more)) }
                        DropdownMenu(expanded = menuFor?.id == h.id, onDismissRequest = { menuFor = null }) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.paste_without_running)) },
                                enabled = canType,
                                onClick = { menuFor = null; controller.paste(h.command); onClose() },
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.copy)) },
                                onClick = { menuFor = null; copyToClipboard(context, h.command); shell.notify(str(R.string.copied_2)) },
                            )
                            DropdownMenuItem(text = { Text(stringResource(R.string.save_as_snippet)) }, onClick = { menuFor = null; saveAs = h })
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.delete)) },
                                onClick = {
                                    menuFor = null
                                    scope.launch {
                                        runCatching { shell.repo.write { deleteCommandHistory(h.id) } }
                                            .onFailure { shell.notify(it.userMessage()) }
                                    }
                                },
                            )
                        }
                    },
                )
                RowDivider()
            }
        }
    }

    saveAs?.let { h ->
        SaveSnippetDialog(shell, h.command, session.hostId, onDone = { saveAs = null })
    }
    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text(stringResource(R.string.clear_command_history)) },
            text = { Text(stringResource(R.string.removes_every_recorded_command_of_this_vault_from)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmClear = false
                        scope.launch {
                            runCatching { shell.repo.write { clearCommandHistory() } }
                                .onFailure { shell.notify(it.userMessage()) }
                        }
                    },
                ) { Text(stringResource(R.string.clear), color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmClear = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

/** Turns a history line into a snippet of the selected (writable) vault. */
@Composable
private fun SaveSnippetDialog(shell: ShellViewModel, command: String, hostId: String?, onDone: () -> Unit) {
    val scope = rememberCoroutineScope()
    val vaults by shell.vaults.collectAsStateWithLifecycle()
    val selectedId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val writable = vaults.filter { !it.locked && it.access != VaultAccess.VIEW }
    var vaultId by remember { mutableStateOf(writable.firstOrNull { it.id == selectedId }?.id ?: writable.firstOrNull()?.id) }
    var label by remember { mutableStateOf(command.take(40).lineSequence().first()) }
    var onlyThisHost by remember { mutableStateOf(false) }
    var busy by remember { mutableStateOf(false) }

    AlertDialog(
        onDismissRequest = onDone,
        title = { Text(stringResource(R.string.save_as_snippet)) },
        text = {
            Column {
                FormField(label, { label = it }, stringResource(R.string.name))
                Spacer(Modifier.height(8.dp))
                Text(command, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, maxLines = 4)
                if (writable.size > 1) {
                    Spacer(Modifier.height(12.dp))
                    SectionLabel(stringResource(R.string.vault))
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        writable.forEach { v ->
                            FilterChip(selected = vaultId == v.id, onClick = { vaultId = v.id }, label = { Text(v.name) })
                        }
                    }
                }
                if (hostId != null) {
                    Spacer(Modifier.height(8.dp))
                    FilterChip(
                        selected = onlyThisHost,
                        onClick = { onlyThisHost = !onlyThisHost },
                        label = { Text(stringResource(R.string.only_for_this_host)) },
                        leadingIcon = if (onlyThisHost) ({ Icon(Icons.Filled.Check, contentDescription = null) }) else null,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !busy && label.isNotBlank() && vaultId != null,
                onClick = {
                    val target = vaultId ?: return@TextButton
                    busy = true
                    scope.launch {
                        runCatching {
                            shell.repo.write {
                                saveSnippet(
                                    SnippetDraft(
                                        id = null,
                                        vaultId = target,
                                        label = label.trim(),
                                        script = command,
                                        packageId = null,
                                        closeAfterRun = false,
                                        targetHostIds = if (onlyThisHost && hostId != null) listOf(hostId) else emptyList(),
                                    ),
                                )
                            }
                        }.onSuccess { shell.notify(str(R.string.snippet_saved)) }
                            .onFailure { shell.notify(it.userMessage()) }
                        onDone()
                    }
                },
            ) { Text(stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDone) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun ThemesTab(shell: ShellViewModel, session: TerminalSession) {
    val scope = rememberCoroutineScope()
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val override by session.themeOverride.collectAsStateWithLifecycle()
    val themes = remember { terminalThemes() }
    val global = remember(settings.terminalTheme) { terminalTheme(settings.terminalTheme) ?: themes.first() }

    fun pick(theme: TerminalTheme?) {
        scope.launch { shell.sessions.applySessionTheme(session, theme, global.palette) }
    }

    LazyColumn(contentPadding = PaddingValues(bottom = 32.dp)) {
        item {
            ListRow(
                title = stringResource(R.string.follow_app_setting),
                subtitle = stringResource(R.string.change_it_under_settings_terminal, global.name),
                leading = { PaletteSwatch(global.palette) },
                modifier = Modifier.clickable { pick(null) },
                trailing = {
                    if (override == null) Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.selected), tint = MaterialTheme.colorScheme.primary)
                },
            )
            SectionLabel(stringResource(R.string.only_this_terminal), Modifier.padding(start = 16.dp, top = 12.dp, bottom = 4.dp))
        }
        items(themes, key = { it.id }) { theme ->
            RowDivider()
            ListRow(
                title = theme.name,
                subtitle = if (theme.dark) stringResource(R.string.dark) else stringResource(R.string.light),
                leading = { PaletteSwatch(theme.palette) },
                modifier = Modifier.clickable { pick(theme) },
                trailing = {
                    if (override == theme.id) Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.selected), tint = MaterialTheme.colorScheme.primary)
                },
            )
        }
    }
}
