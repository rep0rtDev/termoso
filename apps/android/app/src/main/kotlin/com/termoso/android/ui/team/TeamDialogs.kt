package com.termoso.android.ui.team

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.termoso.android.data.userMessage
import com.termoso.android.ui.keychain.copyText
import com.termoso.core.InviteSent
import com.termoso.core.TeamMemberCard
import com.termoso.core.TeamRole
import com.termoso.core.VaultAccess
import com.termoso.core.VaultInfo
import kotlinx.coroutines.launch

/**
 * Invite people: addresses, role, vaults to grant on acceptance. After sending,
 * shows one row per address with a copyable link — the server emails it too
 * when SMTP is configured, but on a self-hosted box it often is not.
 */
@Composable
fun InviteDialog(
    vaults: List<VaultInfo>,
    canMakeAdmin: Boolean,
    onDismiss: () -> Unit,
    onSend: suspend (emails: List<String>, role: TeamRole, vaultIds: List<String>) -> List<InviteSent>,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var text by remember { mutableStateOf("") }
    var role by remember { mutableStateOf(TeamRole.MEMBER) }
    val picked = remember { mutableStateMapOf<String, Boolean>() }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var results by remember { mutableStateOf<List<InviteSent>?>(null) }

    val emails = remember(text) { splitEmails(text) }
    val bad = emails.filterNot(::looksLikeEmail)

    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(if (results == null) "Invite people" else "Invitations sent") },
        text = {
            val sent = results
            if (sent != null) {
                Column(Modifier.verticalScroll(rememberScrollState())) {
                    sent.forEach { r -> InviteResultRow(r, onCopy = { copyText(context, "Invitation link", it) }) }
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Share each link with its recipient. It expires in 14 days and can be revoked from the team screen.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                return@AlertDialog
            }
            Column(Modifier.verticalScroll(rememberScrollState())) {
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text("Email addresses") },
                    placeholder = { Text("one per line, or comma-separated") },
                    minLines = 2,
                    maxLines = 5,
                    enabled = !busy,
                    isError = bad.isNotEmpty(),
                    supportingText = if (bad.isNotEmpty()) {
                        { Text("Not an address: ${bad.first()}") }
                    } else {
                        null
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Email,
                        capitalization = KeyboardCapitalization.None,
                        autoCorrectEnabled = false,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(12.dp))
                Text("Role", style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    listOf(TeamRole.MEMBER, TeamRole.ADMIN).forEach { r ->
                        FilterChip(
                            selected = role == r,
                            onClick = { role = r },
                            label = { Text(r.label()) },
                            enabled = !busy && (r != TeamRole.ADMIN || canMakeAdmin),
                            leadingIcon = if (role == r) {
                                { Icon(Icons.Filled.Check, contentDescription = null, Modifier.size(18.dp)) }
                            } else {
                                null
                            },
                        )
                    }
                }
                Text(role.hint(), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (vaults.isNotEmpty()) {
                    Spacer(Modifier.height(12.dp))
                    Text("Give access to", style = MaterialTheme.typography.labelLarge)
                    vaults.forEach { v ->
                        val on = picked[v.id] == true
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .heightIn(min = 40.dp)
                                .clickable(enabled = !busy && v.access == VaultAccess.MANAGE) { picked[v.id] = !on },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Checkbox(checked = on, onCheckedChange = null, enabled = !busy && v.access == VaultAccess.MANAGE)
                            Spacer(Modifier.size(8.dp))
                            Column {
                                Text(v.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                if (v.access != VaultAccess.MANAGE) {
                                    Text(
                                        "only managers of this vault can grant access",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                    Text(
                        "They can view the chosen vaults once they join and you grant the key.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                error?.let {
                    Spacer(Modifier.height(8.dp))
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
        },
        confirmButton = {
            if (results != null) {
                TextButton(onClick = onDismiss) { Text("Done") }
            } else {
                TextButton(
                    onClick = {
                        scope.launch {
                            busy = true
                            error = null
                            runCatching { onSend(emails, role, picked.filterValues { it }.keys.toList()) }
                                .onSuccess { results = it }
                                .onFailure { error = it.userMessage() }
                            busy = false
                        }
                    },
                    enabled = emails.isNotEmpty() && bad.isEmpty() && !busy,
                ) {
                    if (busy) {
                        CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                    } else {
                        Text(if (emails.size > 1) "Send ${emails.size} invitations" else "Send invitation")
                    }
                }
            }
        },
        dismissButton = {
            if (results == null) TextButton(onClick = onDismiss, enabled = !busy) { Text("Cancel") }
        },
    )
}

@Composable
private fun InviteResultRow(r: InviteSent, onCopy: (String) -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            if (r.error == null) Icons.Filled.Check else Icons.Filled.ErrorOutline,
            contentDescription = null,
            tint = if (r.error == null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            modifier = Modifier.size(20.dp),
        )
        Spacer(Modifier.size(8.dp))
        Column(Modifier.weight(1f)) {
            Text(r.email, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(
                r.error ?: "link ready",
                style = MaterialTheme.typography.bodySmall,
                color = if (r.error == null) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.error,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        r.url?.let { url ->
            IconButton(onClick = { onCopy(url) }) { Icon(Icons.Filled.ContentCopy, contentDescription = "Copy link") }
        }
    }
}

/** New team vault: a name and, optionally, who gets in right away (you are always its manager). */
@Composable
fun NewTeamVaultDialog(
    members: List<TeamMemberCard>,
    onDismiss: () -> Unit,
    onCreate: suspend (name: String, access: Map<String, VaultAccess>) -> Boolean,
) {
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf("") }
    val access = remember { mutableStateMapOf<String, VaultAccess>() }
    var busy by remember { mutableStateOf(false) }
    val others = members.filterNot { it.me }

    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text("New team vault") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("Vault name") },
                    singleLine = true,
                    enabled = !busy,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (others.isNotEmpty()) {
                    Spacer(Modifier.height(12.dp))
                    Text("Members", style = MaterialTheme.typography.labelLarge)
                    others.forEach { m ->
                        Row(Modifier.fillMaxWidth().heightIn(min = 44.dp), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(m.displayName ?: m.email, maxLines = 1, overflow = TextOverflow.Ellipsis)
                                if (m.displayName != null) {
                                    Text(
                                        m.email,
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                }
                            }
                            AccessMenu(
                                current = access[m.userId],
                                allowNone = true,
                                onPick = { a -> if (a == null) access.remove(m.userId) else access[m.userId] = a },
                            ) { open -> AccessChip(access[m.userId], onClick = open) }
                        }
                    }
                }
                Spacer(Modifier.height(8.dp))
                Text(
                    "The vault key is generated on this phone and sealed to each chosen member's account key. " +
                        "The server stores only ciphertext.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = {
                    scope.launch {
                        busy = true
                        val ok = onCreate(name.trim(), access.toMap())
                        busy = false
                        if (ok) onDismiss()
                    }
                },
                enabled = name.isNotBlank() && !busy,
            ) {
                if (busy) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text("Create")
            }
        },
        dismissButton = { TextButton(onClick = onDismiss, enabled = !busy) { Text("Cancel") } },
    )
}
