package com.termoso.android.ui.terminal

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.getSystemService
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.TermosoSwitch
import com.termoso.android.ui.components.UserAvatar
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.LiveParticipantCard
import com.termoso.core.isLiveLink
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Multiplayer sheet for one terminal. Host: start sharing, copy/share the
 * link, grant or take back control per participant, stop. Viewer: who else
 * is here and whether we may type. The link is one opaque string from Rust;
 * the secret in its fragment never leaves it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LiveSheet(session: TerminalSession, shell: ShellViewModel, onClose: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val share by session.share.collectAsStateWithLifecycle()
    val participants by session.participants.collectAsStateWithLifecycle()
    val canWrite by session.canWrite.collectAsStateWithLifecycle()
    var busy by remember { mutableStateOf(false) }

    fun start() {
        busy = true
        scope.launch {
            runCatching { shell.sessions.share(session.id) }
                .onFailure { shell.notify(it.userMessage()) }
            busy = false
        }
    }

    fun stop() {
        busy = true
        scope.launch {
            shell.sessions.stopShare(session.id)
            busy = false
            onClose()
        }
    }

    ModalBottomSheet(onDismissRequest = onClose) {
        Column(Modifier.padding(horizontal = 16.dp).padding(bottom = 24.dp)) {
            Text(
                if (session.isView) stringResource(R.string.shared_terminal) else stringResource(R.string.terminal_sharing),
                style = MaterialTheme.typography.titleMedium,
            )
            Spacer(Modifier.height(4.dp))
            when {
                session.isView -> {
                    Text(
                        if (canWrite) stringResource(R.string.the_host_let_you_type_into_this_terminal) else stringResource(R.string.view_only_ask_the_host_for_control_to),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Participants(shell.repo, participants, null)
                    Spacer(Modifier.height(12.dp))
                    OutlinedButton(
                        onClick = { scope.launch { shell.sessions.close(session.id) }; onClose() },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(stringResource(R.string.leave_2)) }
                }
                share == null -> {
                    Text(
                        stringResource(R.string.let_others_watch_this_terminal_live_through_termoso),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = ::start, enabled = !busy, modifier = Modifier.fillMaxWidth()) {
                        Text(if (busy) stringResource(R.string.starting) else stringResource(R.string.start_sharing))
                    }
                }
                else -> {
                    Text(
                        stringResource(R.string.anyone_with_the_link_and_a_termoso_account),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val link = share?.link().orEmpty()
                        OutlinedButton(
                            onClick = {
                                context.getSystemService<ClipboardManager>()
                                    ?.setPrimaryClip(ClipData.newPlainText(str(R.string.termoso_link), link))
                                shell.notify(str(R.string.link_copied))
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text(stringResource(R.string.copy_link))
                        }
                        Button(onClick = { shareLink(context, link) }, modifier = Modifier.weight(1f)) {
                            Icon(Icons.Filled.Share, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text(stringResource(R.string.share))
                        }
                    }
                    Participants(shell.repo, participants) { p, enabled ->
                        scope.launch {
                            runCatching { shell.sessions.setControl(session.id, p.userId, enabled) }
                                .onFailure { shell.notify(it.userMessage()) }
                        }
                    }
                    Spacer(Modifier.height(12.dp))
                    OutlinedButton(onClick = ::stop, enabled = !busy, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.stop_sharing))
                    }
                }
            }
        }
    }
}

@Composable
private fun Participants(
    repo: VaultRepository,
    participants: List<LiveParticipantCard>,
    onControl: ((LiveParticipantCard, Boolean) -> Unit)?,
) {
    SectionLabel(if (participants.isEmpty()) stringResource(R.string.participants) else stringResource(R.string.participants_2, participants.size))
    if (participants.isEmpty()) {
        Text(
            stringResource(R.string.nobody_has_joined_yet),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    SectionCard {
        participants.forEachIndexed { i, p ->
            if (i > 0) RowDivider()
            val name = p.displayName?.takeIf { it.isNotBlank() } ?: p.email
            val role = buildList {
                if (p.isHost) add(stringResource(R.string.host))
                if (p.me) add("you")
                if (!p.isHost) add(if (p.canWrite) stringResource(R.string.can_type) else stringResource(R.string.view_only))
            }.joinToString(" · ")
            val avatar: @Composable () -> Unit = {
                UserAvatar(repo, userId = p.userId, tag = p.avatar, name = name)
            }
            if (onControl != null && !p.isHost) {
                key(p.userId) {
                    // The grant round-trips through the server; show the
                    // requested state until the participant list catches up.
                    var pending by remember { mutableStateOf<Boolean?>(null) }
                    LaunchedEffect(p.canWrite) { pending = null }
                    LaunchedEffect(pending) {
                        if (pending != null) {
                            delay(4_000)
                            pending = null
                        }
                    }
                    ListRow(
                        title = name,
                        subtitle = role,
                        leading = avatar,
                        modifier = Modifier.toggleable(
                            value = pending ?: p.canWrite,
                            role = Role.Switch,
                            onValueChange = { on -> pending = on; onControl(p, on) },
                        ),
                    ) {
                        TermosoSwitch(checked = pending ?: p.canWrite, onCheckedChange = null)
                    }
                }
            } else {
                ListRow(title = name, subtitle = role, leading = avatar)
            }
        }
    }
}

private fun shareLink(context: Context, link: String) {
    val send = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_SUBJECT, str(R.string.join_my_terminal_in_termoso))
        putExtra(Intent.EXTRA_TEXT, link)
    }
    context.startActivity(Intent.createChooser(send, str(R.string.share_link)))
}

/** Paste or type a join link (`https://…/join/…` or `termoso://join/…`); Rust validates it on Join. */
@Composable
fun JoinLiveDialog(onDismiss: () -> Unit, onJoin: (String) -> Unit) {
    val context = LocalContext.current
    var link by remember {
        val clip = context.getSystemService<ClipboardManager>()?.primaryClip
        val text = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()?.trim()
        mutableStateOf(text?.takeIf { isLiveLink(it) }.orEmpty())
    }
    val valid = isLiveLink(link.trim())
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.join_shared_terminal)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(R.string.paste_the_join_link_the_host_sent_you),
                    style = MaterialTheme.typography.bodyMedium,
                )
                OutlinedTextField(
                    value = link,
                    onValueChange = { link = it },
                    label = { Text(stringResource(R.string.link)) },
                    singleLine = true,
                    isError = link.isNotBlank() && !valid,
                    modifier = Modifier.fillMaxWidth(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        capitalization = KeyboardCapitalization.None,
                        imeAction = ImeAction.Go,
                        autoCorrectEnabled = false,
                    ),
                )
            }
        },
        confirmButton = { TextButton(onClick = { onJoin(link.trim()) }, enabled = valid) { Text(stringResource(R.string.join)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
