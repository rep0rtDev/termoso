package com.termoso.android.ui.keychain

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Save
import androidx.compose.material.icons.filled.Security
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.KeyItem
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class Sheet { RENAME, PASSPHRASE, CERTIFICATE, EXPORT, DELETE }

/** Details and actions for one stored SSH key. */
@Composable
fun KeyDetailScreen(shell: ShellViewModel, keyId: String, onBack: () -> Unit) {
    val vm: KeyDetailViewModel = viewModel(key = "key/$keyId") { KeyDetailViewModel(shell.repo, keyId) }
    val s by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    var sheet by remember { mutableStateOf<Sheet?>(null) }

    LaunchedEffect(s.notice) { s.notice?.let { shell.notify(it); vm.noticeShown() } }
    LaunchedEffect(s.deleted) { if (s.deleted) onBack() }

    val key = s.key
    SubScreen(
        key?.label ?: "Key",
        onBack,
        actions = {
            if (key != null) {
                IconButton(onClick = { sheet = Sheet.RENAME }) { Icon(Icons.Filled.Edit, contentDescription = "Rename") }
            }
        },
    ) { padding ->
        if (key == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                if (s.loading) CircularProgressIndicator()
            }
            return@SubScreen
        }
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            SectionCard {
                ListRow(
                    title = key.label,
                    subtitle = keyTypeLabel(key.keyType, key.bits) + if (key.usedBy > 0u) " · used by ${key.usedBy} host${if (key.usedBy == 1u) "" else "s"}" else "",
                    leading = { IconTile(if (s.securityKey != null) Icons.Filled.Security else Icons.Filled.Key, selected = true) },
                )
                if (key.fingerprint.isNotBlank()) {
                    RowDivider()
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text("Fingerprint", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(key.fingerprint, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                    }
                }
                if (key.comment.isNotBlank()) {
                    RowDivider()
                    ListRow(title = "Comment", subtitle = key.comment)
                }
                if (key.unreadable) {
                    RowDivider()
                    ListRow(
                        title = "Cannot read this key",
                        subtitle = "The stored material is not a supported private key. Delete it and import again.",
                        titleColor = MaterialTheme.colorScheme.error,
                    )
                }
            }

            SectionLabel("Public key")
            SectionCard {
                Text(
                    s.publicKey.ifBlank { "—" },
                    style = MaterialTheme.typography.bodySmall,
                    fontFamily = FontFamily.Monospace,
                    modifier = Modifier.padding(16.dp),
                )
                RowDivider()
                Row(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
                    TextButton(
                        enabled = s.publicKey.isNotBlank(),
                        onClick = { copyText(context, "Public key", s.publicKey); shell.notify("Public key copied") },
                    ) {
                        Icon(Icons.Filled.ContentCopy, contentDescription = null, Modifier.height(18.dp))
                        Text("  Copy public key")
                    }
                }
            }

            s.securityKey?.let { sk ->
                SectionLabel("Security key")
                SectionCard {
                    ListRow(
                        title = "Private key lives on the security key",
                        subtitle = "Only the public key and a handle are stored here; the token must be plugged in or held to the phone to connect.",
                    )
                    RowDivider()
                    ListRow(title = "Application", subtitle = sk.application)
                    RowDivider()
                    ListRow(
                        title = "On connect",
                        subtitle = listOfNotNull(
                            when (sk.userPresence) { true -> "touch required"; false -> "no touch"; null -> null },
                            when (sk.userVerification) { true -> "PIN required"; false -> null; null -> null },
                        ).joinToString(", ").ifBlank { "Unknown" },
                    )
                    RowDivider()
                    ListRow(
                        title = "Resident key",
                        subtitle = when (sk.resident) {
                            true -> "Stored on the token, can be loaded elsewhere with the PIN"
                            false -> "Not stored on the token — only this handle unlocks it"
                            null -> "Unknown"
                        },
                    )
                    sk.credentialId?.let { cred ->
                        RowDivider()
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text("Credential ID", style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(cred, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }

            SectionLabel("Security")
            SectionCard {
                ChevronRow(
                    title = if (key.encrypted) "Change passphrase" else "Set passphrase",
                    subtitle = when {
                        !key.encrypted && s.securityKey != null -> "Key handle is stored without a passphrase"
                        !key.encrypted -> "Private key is stored without a passphrase"
                        key.hasPassphrase -> "Passphrase remembered in the vault"
                        else -> "Prompted on every connection"
                    },
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.PASSPHRASE },
                )
                RowDivider()
                ChevronRow(
                    title = if (key.hasCertificate) "Certificate" else "Attach certificate",
                    subtitle = if (key.hasCertificate) "OpenSSH certificate attached" else "Optional signed public key from your CA",
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.CERTIFICATE },
                )
                RowDivider()
                ChevronRow(
                    title = if (s.securityKey != null) "Export key handle" else "Export private key",
                    subtitle = if (s.securityKey != null) "Useless without the security key — confirmation required" else "Reveals the secret — confirmation required",
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.EXPORT },
                )
            }

            SectionLabel(" ")
            SectionCard {
                ListRow(
                    title = "Delete key",
                    subtitle = if (key.usedBy > 0u) "Hosts using it will fall back to password or prompt" else null,
                    titleColor = MaterialTheme.colorScheme.error,
                    modifier = Modifier.clickable { sheet = Sheet.DELETE },
                )
            }
        }
    }

    when (sheet) {
        Sheet.RENAME -> RenameDialog(key?.label ?: "", onDismiss = { sheet = null }) { vm.rename(it); sheet = null }
        Sheet.PASSPHRASE -> key?.let { k ->
            PassphraseDialog(k, onDismiss = { sheet = null }) { cur, next, remember ->
                vm.changePassphrase(cur, next, remember)
                sheet = null
            }
        }
        Sheet.CERTIFICATE -> key?.let { k ->
            CertificateDialog(k, onNotify = shell::notify, onDismiss = { sheet = null }) { text ->
                vm.setCertificate(text)
                sheet = null
            }
        }
        Sheet.EXPORT -> key?.let { k ->
            ExportDialog(k, vm, onNotify = shell::notify, onDismiss = { sheet = null })
        }
        Sheet.DELETE -> ConfirmDialog(
            title = "Delete key?",
            text = "\"${key?.label}\" will be removed from the vault." +
                if ((key?.usedBy ?: 0u) > 0u) " ${key?.usedBy} host(s) reference it." else "",
            confirm = "Delete",
            onConfirm = { sheet = null; vm.delete() },
            onDismiss = { sheet = null },
        )
        null -> {}
    }
}

@Composable
private fun RenameDialog(current: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var label by remember { mutableStateOf(current) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Rename key") },
        text = { FormField(label, { label = it }, "Label") },
        confirmButton = { TextButton(enabled = label.isNotBlank(), onClick = { onSave(label) }) { Text("Save") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun PassphraseDialog(key: KeyItem, onDismiss: () -> Unit, onSave: (String?, String?, Boolean) -> Unit) {
    val needCurrent = key.encrypted && !key.hasPassphrase
    var current by remember { mutableStateOf("") }
    var next by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var remember by remember { mutableStateOf(true) }
    var removing by remember { mutableStateOf(false) }
    val mismatch = next.isNotEmpty() && confirm.isNotEmpty() && next != confirm
    val valid = (!needCurrent || current.isNotEmpty()) && (removing || (next.isNotEmpty() && next == confirm))
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (key.encrypted) "Change passphrase" else "Set passphrase") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (needCurrent) SecretField(current, { current = it }, "Current passphrase")
                if (key.encrypted) {
                    SwitchRow(title = "Remove passphrase", checked = removing, onCheckedChange = { removing = it })
                }
                if (!removing) {
                    SecretField(next, { next = it }, "New passphrase")
                    SecretField(confirm, { confirm = it }, "Confirm passphrase")
                    if (mismatch) Text("Passphrases do not match", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    SwitchRow(title = "Remember passphrase", checked = remember, onCheckedChange = { remember = it })
                }
            }
        },
        confirmButton = {
            TextButton(enabled = valid, onClick = {
                onSave(current.takeIf { needCurrent }, if (removing) null else next, remember && !removing)
            }) { Text(if (removing) "Remove" else "Save") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
private fun CertificateDialog(key: KeyItem, onNotify: (String) -> Unit, onDismiss: () -> Unit, onSave: (String?) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var text by remember { mutableStateOf("") }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            runCatching { readTextFile(context, uri) }
                .onSuccess { text = it }
                .onFailure { onNotify(it.message ?: "Could not read the file.") }
        }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (key.hasCertificate) "Replace certificate" else "Attach certificate") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    "Paste the OpenSSH certificate issued for this key (usually `<key>-cert.pub`). Rust checks that it matches before saving.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                KeyTextArea(text, { text = it }, placeholder = "ssh-ed25519-cert-v01@openssh.com AAAA…", minLines = 3)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { pasteText(context)?.let { text = it } ?: onNotify("Clipboard is empty") }) {
                        Icon(Icons.Filled.ContentPaste, contentDescription = null, Modifier.height(18.dp))
                        Text("  Paste")
                    }
                    TextButton(onClick = { pick.launch(arrayOf("*/*")) }) {
                        Icon(Icons.Filled.FolderOpen, contentDescription = null, Modifier.height(18.dp))
                        Text("  Open file")
                    }
                }
            }
        },
        confirmButton = { TextButton(enabled = text.isNotBlank(), onClick = { onSave(text) }) { Text("Save") } },
        dismissButton = {
            Row {
                if (key.hasCertificate) {
                    TextButton(onClick = { onSave(null) }) { Text("Remove", color = MaterialTheme.colorScheme.error) }
                }
                TextButton(onClick = onDismiss) { Text("Cancel") }
            }
        },
    )
}

/**
 * Two-step private-key export: confirm intent, then (optionally) unlock and
 * re-encrypt. The text goes to the clipboard flagged sensitive or straight to a
 * file the user picks; it is never shown on screen or kept in state.
 */
@Composable
private fun ExportDialog(key: KeyItem, vm: KeyDetailViewModel, onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirmed by remember { mutableStateOf(false) }
    var passphrase by remember { mutableStateOf("") }
    var exportPassphrase by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val needPassphrase = key.encrypted && !key.hasPassphrase

    suspend fun export(): String? {
        busy = true
        val text = vm.exportPrivate(
            passphrase = passphrase.takeIf { needPassphrase && it.isNotEmpty() },
            exportPassphrase = exportPassphrase.takeIf { it.isNotEmpty() },
        )
        busy = false
        return text
    }

    val saveFile = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            val text = export() ?: return@launch
            runCatching {
                withContext(Dispatchers.IO) {
                    context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) } ?: error("Could not write the file.")
                }
            }.onSuccess { onNotify("Private key saved"); onDismiss() }
                .onFailure { onNotify(it.message ?: "Could not write the file.") }
        }
    }

    if (!confirmed) {
        AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text("Export private key?") },
            text = {
                Text(
                    "Anyone with the private key can log in as you on every host that trusts it. " +
                        "Export only to a device you control, and prefer setting an export passphrase.",
                )
            },
            confirmButton = { TextButton(onClick = { confirmed = true }) { Text("I understand", color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
        )
        return
    }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text("Export \"${key.label}\"") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (needPassphrase) SecretField(passphrase, { passphrase = it }, "Key passphrase")
                SecretField(exportPassphrase, { exportPassphrase = it }, "Export passphrase (optional)")
                Text(
                    if (exportPassphrase.isEmpty()) "The exported file will be unencrypted OpenSSH text." else "The export is re-encrypted with this passphrase.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
        confirmButton = {
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(enabled = !busy && (!needPassphrase || passphrase.isNotEmpty()), onClick = {
                    saveFile.launch(key.label.replace(Regex("[^A-Za-z0-9._-]+"), "_").ifBlank { "id_key" })
                }) {
                    Icon(Icons.Filled.Save, contentDescription = null, Modifier.height(18.dp))
                    Text("  File")
                }
                TextButton(enabled = !busy && (!needPassphrase || passphrase.isNotEmpty()), onClick = {
                    scope.launch {
                        val text = export() ?: return@launch
                        copyText(context, "Private key", text, sensitive = true)
                        onNotify("Private key copied — clear the clipboard when done")
                        onDismiss()
                    }
                }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = null, Modifier.height(18.dp))
                    Text("  Copy")
                }
            }
        },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text("Cancel") } },
    )
}
