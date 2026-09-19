package com.termoso.android.ui.keychain

import androidx.compose.ui.res.pluralStringResource
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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.str
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
        key?.label ?: stringResource(R.string.key),
        onBack,
        actions = {
            if (key != null) {
                IconButton(onClick = { sheet = Sheet.RENAME }) { Icon(Icons.Filled.Edit, contentDescription = stringResource(R.string.rename)) }
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
                    subtitle = keyTypeLabel(key.keyType, key.bits) + if (key.usedBy > 0u) pluralStringResource(R.plurals.sep_used_by_hosts, key.usedBy.toInt(), key.usedBy.toInt()) else "",
                    leading = { IconTile(if (s.securityKey != null) Icons.Filled.Security else Icons.Filled.Key, selected = true) },
                )
                if (key.fingerprint.isNotBlank()) {
                    RowDivider()
                    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(stringResource(R.string.fingerprint), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        Text(key.fingerprint, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                    }
                }
                if (key.comment.isNotBlank()) {
                    RowDivider()
                    ListRow(title = stringResource(R.string.comment), subtitle = key.comment)
                }
                if (key.unreadable) {
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.cannot_read_this_key),
                        subtitle = stringResource(R.string.the_stored_material_is_not_a_supported_private),
                        titleColor = MaterialTheme.colorScheme.error,
                    )
                }
            }

            SectionLabel(stringResource(R.string.public_key_2))
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
                        onClick = { copyText(context, str(R.string.public_key_2), s.publicKey); shell.notify(str(R.string.public_key_copied)) },
                    ) {
                        Icon(Icons.Filled.ContentCopy, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.copy_public_key_2))
                    }
                }
            }

            s.securityKey?.let { sk ->
                SectionLabel(stringResource(R.string.security_key))
                SectionCard {
                    ListRow(
                        title = stringResource(R.string.private_key_lives_on_the_security_key),
                        subtitle = stringResource(R.string.only_the_public_key_and_a_handle_are),
                    )
                    RowDivider()
                    ListRow(title = stringResource(R.string.application), subtitle = sk.application)
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.on_connect),
                        subtitle = listOfNotNull(
                            when (sk.userPresence) { true -> stringResource(R.string.touch_required); false -> stringResource(R.string.no_touch); null -> null },
                            when (sk.userVerification) { true -> stringResource(R.string.pin_required); false -> null; null -> null },
                        ).joinToString(", ").ifBlank { stringResource(R.string.unknown) },
                    )
                    RowDivider()
                    ListRow(
                        title = stringResource(R.string.resident_key),
                        subtitle = when (sk.resident) {
                            true -> stringResource(R.string.stored_on_the_token_can_be_loaded_elsewhere)
                            false -> stringResource(R.string.not_stored_on_the_token_only_this_handle)
                            null -> stringResource(R.string.unknown)
                        },
                    )
                    sk.credentialId?.let { cred ->
                        RowDivider()
                        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            Text(stringResource(R.string.credential_id), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(cred, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }

            SectionLabel(stringResource(R.string.security))
            SectionCard {
                ChevronRow(
                    title = if (key.encrypted) stringResource(R.string.change_passphrase) else stringResource(R.string.set_passphrase),
                    subtitle = when {
                        !key.encrypted && s.securityKey != null -> stringResource(R.string.key_handle_is_stored_without_a_passphrase)
                        !key.encrypted -> stringResource(R.string.private_key_is_stored_without_a_passphrase)
                        key.hasPassphrase -> stringResource(R.string.passphrase_remembered_in_the_vault)
                        else -> stringResource(R.string.prompted_on_every_connection)
                    },
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.PASSPHRASE },
                )
                RowDivider()
                ChevronRow(
                    title = if (key.hasCertificate) stringResource(R.string.certificate) else stringResource(R.string.attach_certificate),
                    subtitle = if (key.hasCertificate) stringResource(R.string.openssh_certificate_attached) else stringResource(R.string.optional_signed_public_key_from_your_ca),
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.CERTIFICATE },
                )
                RowDivider()
                ChevronRow(
                    title = if (s.securityKey != null) stringResource(R.string.export_key_handle) else stringResource(R.string.export_private_key),
                    subtitle = if (s.securityKey != null) stringResource(R.string.useless_without_the_security_key_confirmation_required) else stringResource(R.string.reveals_the_secret_confirmation_required),
                    modifier = Modifier.clickable(enabled = !key.unreadable) { sheet = Sheet.EXPORT },
                )
            }

            SectionLabel(" ")
            SectionCard {
                ListRow(
                    title = stringResource(R.string.delete_key),
                    subtitle = if (key.usedBy > 0u) stringResource(R.string.hosts_using_it_will_fall_back_to_password) else null,
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
            title = stringResource(R.string.delete_key_2),
            text = stringResource(R.string.will_be_removed_from_the_vault, key?.label.orEmpty()) +
                if ((key?.usedBy ?: 0u) > 0u) stringResource(R.string.host_s_reference_it, (key?.usedBy ?: 0u).toInt()) else "",
            confirm = stringResource(R.string.delete),
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
        title = { Text(stringResource(R.string.rename_key)) },
        text = { FormField(label, { label = it }, stringResource(R.string.label)) },
        confirmButton = { TextButton(enabled = label.isNotBlank(), onClick = { onSave(label) }) { Text(stringResource(R.string.save)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
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
        title = { Text(if (key.encrypted) stringResource(R.string.change_passphrase) else stringResource(R.string.set_passphrase)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (needCurrent) SecretField(current, { current = it }, stringResource(R.string.current_passphrase))
                if (key.encrypted) {
                    SwitchRow(title = stringResource(R.string.remove_passphrase), checked = removing, onCheckedChange = { removing = it })
                }
                if (!removing) {
                    SecretField(next, { next = it }, stringResource(R.string.new_passphrase))
                    SecretField(confirm, { confirm = it }, stringResource(R.string.confirm_passphrase))
                    if (mismatch) Text(stringResource(R.string.passphrases_do_not_match), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    SwitchRow(title = stringResource(R.string.remember_passphrase), checked = remember, onCheckedChange = { remember = it })
                }
            }
        },
        confirmButton = {
            TextButton(enabled = valid, onClick = {
                onSave(current.takeIf { needCurrent }, if (removing) null else next, remember && !removing)
            }) { Text(if (removing) stringResource(R.string.remove_2) else stringResource(R.string.save)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
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
                .onFailure { onNotify(it.message ?: str(R.string.could_not_read_the_file)) }
        }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (key.hasCertificate) stringResource(R.string.replace_certificate) else stringResource(R.string.attach_certificate)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(R.string.paste_the_openssh_certificate_issued_for_this_key),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                KeyTextArea(text, { text = it }, placeholder = "ssh-ed25519-cert-v01@openssh.com AAAA…", minLines = 3)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { pasteText(context)?.let { text = it } ?: onNotify(str(R.string.clipboard_is_empty)) }) {
                        Icon(Icons.Filled.ContentPaste, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.paste))
                    }
                    TextButton(onClick = { pick.launch(arrayOf("*/*")) }) {
                        Icon(Icons.Filled.FolderOpen, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.open_file))
                    }
                }
            }
        },
        confirmButton = { TextButton(enabled = text.isNotBlank(), onClick = { onSave(text) }) { Text(stringResource(R.string.save)) } },
        dismissButton = {
            Row {
                if (key.hasCertificate) {
                    TextButton(onClick = { onSave(null) }) { Text(stringResource(R.string.remove_2), color = MaterialTheme.colorScheme.error) }
                }
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
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
                    context.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray()) } ?: error(str(R.string.could_not_write_the_file))
                }
            }.onSuccess { onNotify(str(R.string.private_key_saved)); onDismiss() }
                .onFailure { onNotify(it.message ?: str(R.string.could_not_write_the_file)) }
        }
    }

    if (!confirmed) {
        AlertDialog(
            onDismissRequest = onDismiss,
            title = { Text(stringResource(R.string.export_private_key_2)) },
            text = {
                Text(
                    stringResource(R.string.anyone_with_the_private_key_can_log_in),
                )
            },
            confirmButton = { TextButton(onClick = { confirmed = true }) { Text(stringResource(R.string.i_understand), color = MaterialTheme.colorScheme.error) } },
            dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
        )
        return
    }
    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        title = { Text(stringResource(R.string.export, key.label)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                if (needPassphrase) SecretField(passphrase, { passphrase = it }, stringResource(R.string.key_passphrase))
                SecretField(exportPassphrase, { exportPassphrase = it }, stringResource(R.string.export_passphrase_optional))
                Text(
                    if (exportPassphrase.isEmpty()) stringResource(R.string.the_exported_file_will_be_unencrypted_openssh_text) else stringResource(R.string.the_export_is_re_encrypted_with_this_passphrase),
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
                    Text(stringResource(R.string.file_))
                }
                TextButton(enabled = !busy && (!needPassphrase || passphrase.isNotEmpty()), onClick = {
                    scope.launch {
                        val text = export() ?: return@launch
                        copyText(context, str(R.string.private_key), text, sensitive = true)
                        onNotify(str(R.string.private_key_copied_clear_the_clipboard_when_done))
                        onDismiss()
                    }
                }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = null, Modifier.height(18.dp))
                    Text(stringResource(R.string.copy_2))
                }
            }
        },
        dismissButton = { TextButton(enabled = !busy, onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
