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
import androidx.compose.material3.Switch
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.getSystemService
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.android.data.VaultRepository
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
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
                if (session.isView) "Shared terminal" else "Terminal sharing",
                style = MaterialTheme.typography.titleMedium,
            )
            Spacer(Modifier.height(4.dp))
            when {
                session.isView -> {
                    Text(
                        if (canWrite) "The host let you type into this terminal." else "View only. Ask the host for control to type.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Participants(shell.repo, participants, null)
                    Spacer(Modifier.height(12.dp))
                    OutlinedButton(
                        onClick = { scope.launch { shell.sessions.close(session.id) }; onClose() },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text("Leave") }
                }
                share == null -> {
                    Text(
                        "Let others watch this terminal live through Termoso, end-to-end encrypted. " +
                            "You choose who may type; stop any time.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = ::start, enabled = !busy, modifier = Modifier.fillMaxWidth()) {
                        Text(if (busy) "Starting…" else "Start sharing")
                    }
                }
                else -> {
                    Text(
                        "Anyone with the link and a Termoso account can watch. " +
                            "It contains the session key, so share it only with people you trust.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(12.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        val link = share?.link().orEmpty()
                        OutlinedButton(
                            onClick = {
                                context.getSystemService<ClipboardManager>()
                                    ?.setPrimaryClip(ClipData.newPlainText("Termoso link", link))
                                shell.notify("Link copied")
                            },
                            modifier = Modifier.weight(1f),
                        ) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text("Copy link")
                        }
                        Button(onClick = { shareLink(context, link) }, modifier = Modifier.weight(1f)) {
                            Icon(Icons.Filled.Share, contentDescription = null)
                            Spacer(Modifier.width(8.dp))
                            Text("Share")
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
                        Text("Stop sharing")
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
    SectionLabel(if (participants.isEmpty()) "Participants" else "Participants · ${participants.size}")
    if (participants.isEmpty()) {
        Text(
            "Nobody has joined yet.",
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
                if (p.isHost) add("Host")
                if (p.me) add("you")
                if (!p.isHost) add(if (p.canWrite) "can type" else "view only")
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
                        Switch(checked = pending ?: p.canWrite, onCheckedChange = null)
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
        putExtra(Intent.EXTRA_SUBJECT, "Join my terminal in Termoso")
        putExtra(Intent.EXTRA_TEXT, link)
    }
    context.startActivity(Intent.createChooser(send, "Share link"))
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
        title = { Text("Join shared terminal") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Paste the join link the host sent you. You will watch their terminal; typing needs their permission.",
                    style = MaterialTheme.typography.bodyMedium,
                )
                OutlinedTextField(
                    value = link,
                    onValueChange = { link = it },
                    label = { Text("Link") },
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
        confirmButton = { TextButton(onClick = { onJoin(link.trim()) }, enabled = valid) { Text("Join") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
