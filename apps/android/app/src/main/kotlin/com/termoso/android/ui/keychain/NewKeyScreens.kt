package com.termoso.android.ui.keychain

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.R
import com.termoso.android.str
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.PickerRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SecretField
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.vault.vaultLabel
import com.termoso.core.KeyPreview
import com.termoso.core.VaultInfo
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun EditorScaffold(
    title: String,
    working: Boolean,
    canSave: Boolean,
    onClose: () -> Unit,
    onSave: () -> Unit,
    content: @Composable () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = { IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close)) } },
                actions = {
                    IconButton(onClick = onSave, enabled = canSave) {
                        if (working) {
                            CircularProgressIndicator(modifier = Modifier.height(20.dp), strokeWidth = 2.dp)
                        } else {
                            Icon(Icons.Filled.Check, contentDescription = stringResource(R.string.save))
                        }
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) { content() }
    }
}

@Composable
internal fun VaultPicker(vaults: List<VaultInfo>, selected: String?, onPick: (String) -> Unit) {
    if (vaults.size < 2) return
    PickerRow(
        label = stringResource(R.string.vault),
        value = vaults.firstOrNull { it.id == selected }?.let(::vaultLabel) ?: "",
        options = vaults.map { it.id to vaultLabel(it) },
        selected = selected,
        onPick = { id -> id?.let(onPick) },
        empty = null,
    )
}

/** Generate an Ed25519 / RSA / ECDSA key pair inside the encrypted vault. */
@Composable
fun GenerateKeyScreen(shell: ShellViewModel, onClose: () -> Unit, onSaved: (String) -> Unit) {
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: GenerateKeyViewModel = viewModel { GenerateKeyViewModel(shell.repo, initialVault) }
    val s by vm.state.collectAsStateWithLifecycle()

    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.savedId) { s.savedId?.let(onSaved) }

    EditorScaffold(stringResource(R.string.generate_key), s.working, s.canSave, onClose, vm::generate) {
        VaultPicker(s.vaults, s.vaultId) { id -> vm.update { it.copy(vaultId = id) } }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(s.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.label), placeholder = stringResource(R.string.my_laptop))
                FormField(s.comment, { v -> vm.update { it.copy(comment = v) } }, stringResource(R.string.comment), placeholder = "user@host")
            }
        }

        SectionLabel(stringResource(R.string.algorithm))
        SectionCard {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                KeyKind.entries.forEach { kind ->
                    FilterChip(
                        selected = s.kind == kind,
                        onClick = { vm.update { it.copy(kind = kind) } },
                        label = { Text(kind.label) },
                    )
                }
            }
            Text(
                stringResource(s.kind.hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
            )
            when (s.kind) {
                KeyKind.RSA -> BitsRow(listOf(2048u, 3072u, 4096u), s.rsaBits) { b -> vm.update { it.copy(rsaBits = b) } }
                KeyKind.ECDSA -> BitsRow(listOf(256u, 384u), s.ecdsaBits, prefix = "P-") { b -> vm.update { it.copy(ecdsaBits = b) } }
                KeyKind.ED25519 -> {}
            }
        }

        SectionLabel(stringResource(R.string.passphrase_2))
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SecretField(s.passphrase, { v -> vm.update { it.copy(passphrase = v) } }, stringResource(R.string.passphrase_optional))
                if (s.passphrase.isNotEmpty()) {
                    SecretField(s.confirm, { v -> vm.update { it.copy(confirm = v) } }, stringResource(R.string.confirm_passphrase))
                    if (s.passphraseMismatch) {
                        Text(stringResource(R.string.passphrases_do_not_match), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            if (s.passphrase.isNotEmpty()) {
                RowDivider()
                SwitchRow(
                    title = stringResource(R.string.remember_passphrase),
                    subtitle = stringResource(R.string.stored_encrypted_in_the_vault_so_connections_do),
                    checked = s.remember,
                    onCheckedChange = { v -> vm.update { it.copy(remember = v) } },
                )
            }
        }
        Text(
            stringResource(R.string.the_private_key_is_generated_on_this_device),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 4.dp),
        )
    }
}

@Composable
private fun BitsRow(options: List<UInt>, selected: UInt, prefix: String = "", onPick: (UInt) -> Unit) {
    RowDivider()
    Row(Modifier.padding(horizontal = 16.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { b ->
            FilterChip(selected = selected == b, onClick = { onPick(b) }, label = { Text("$prefix$b") })
        }
    }
}

/** Paste or open a private key file, inspect it in Rust, then store it. */
@Composable
fun ImportKeyScreen(shell: ShellViewModel, onClose: () -> Unit, onSaved: (String) -> Unit) {
    val initialVault by shell.selectedVaultId.collectAsStateWithLifecycle()
    val vm: ImportKeyViewModel = viewModel { ImportKeyViewModel(shell.repo, initialVault) }
    val s by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    LaunchedEffect(s.error) { s.error?.let { shell.notify(it); vm.errorShown() } }
    LaunchedEffect(s.savedId) { s.savedId?.let(onSaved) }

    val pickKey = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            runCatching { readTextFile(context, uri) }
                .onSuccess(vm::setPrivateKey)
                .onFailure { shell.notify(it.message ?: str(R.string.could_not_read_the_file)) }
        }
    }
    val pickCert = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        scope.launch {
            runCatching { readTextFile(context, uri) }
                .onSuccess { text -> vm.update { it.copy(certificate = text) } }
                .onFailure { shell.notify(it.message ?: str(R.string.could_not_read_the_file)) }
        }
    }

    EditorScaffold(stringResource(R.string.import_key), s.working, s.canSave, onClose, vm::import) {
        VaultPicker(s.vaults, s.vaultId) { id -> vm.update { it.copy(vaultId = id) } }
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                FormField(s.label, { v -> vm.update { it.copy(label = v) } }, stringResource(R.string.label), placeholder = s.preview?.comment?.ifBlank { null } ?: stringResource(R.string.imported_key))
            }
        }

        SectionLabel(stringResource(R.string.private_key))
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                KeyTextArea(
                    value = s.privateKey,
                    onChange = vm::setPrivateKey,
                    placeholder = stringResource(R.string.begin_openssh_private_key_openssh_pem_pkcs_8),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { pasteText(context)?.let(vm::setPrivateKey) ?: shell.notify(str(R.string.clipboard_is_empty)) }) {
                        Icon(Icons.Filled.ContentPaste, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.paste))
                    }
                    TextButton(onClick = { pickKey.launch(arrayOf("*/*")) }) {
                        Icon(Icons.Filled.FolderOpen, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.open_file))
                    }
                }
            }
            val preview = s.preview
            val previewError = s.previewError
            if (preview != null || previewError != null) {
                RowDivider()
                PreviewBlock(preview, previewError)
            }
        }

        if (s.preview?.encrypted == true) {
            SectionLabel(stringResource(R.string.passphrase_2))
            SectionCard {
                Column(Modifier.padding(16.dp)) {
                    SecretField(s.passphrase, { v -> vm.update { it.copy(passphrase = v) } }, stringResource(R.string.key_passphrase))
                }
                RowDivider()
                SwitchRow(
                    title = stringResource(R.string.remember_passphrase),
                    subtitle = stringResource(R.string.stored_encrypted_in_the_vault_so_connections_do),
                    checked = s.remember,
                    onCheckedChange = { v -> vm.update { it.copy(remember = v) } },
                )
            }
        }

        SectionLabel(stringResource(R.string.certificate_optional))
        SectionCard {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                KeyTextArea(
                    value = s.certificate,
                    onChange = { v -> vm.update { it.copy(certificate = v) } },
                    placeholder = "ssh-ed25519-cert-v01@openssh.com AAAA…",
                    minLines = 2,
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { pasteText(context)?.let { t -> vm.update { it.copy(certificate = t) } } ?: shell.notify(str(R.string.clipboard_is_empty)) }) {
                        Icon(Icons.Filled.ContentPaste, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.paste))
                    }
                    TextButton(onClick = { pickCert.launch(arrayOf("*/*")) }) {
                        Icon(Icons.Filled.FolderOpen, contentDescription = null, Modifier.height(18.dp))
                        Text(stringResource(R.string.open_file))
                    }
                }
            }
        }
    }
}

@Composable
internal fun KeyTextArea(value: String, onChange: (String) -> Unit, placeholder: String, minLines: Int = 4) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        placeholder = { Text(placeholder, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall) },
        textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
        minLines = minLines,
        maxLines = 10,
        keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
        modifier = Modifier.fillMaxWidth().heightIn(min = (minLines * 20 + 16).dp),
    )
}

@Composable
private fun PreviewBlock(preview: KeyPreview?, error: String?) {
    Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        if (preview == null) {
            Text(error ?: "", color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            return
        }
        Text(
            listOfNotNull(
                keyTypeLabel(preview.keyType, preview.bits),
                if (preview.putty) stringResource(R.string.putty_ppk) else null,
                if (preview.encrypted) stringResource(R.string.encrypted) else stringResource(R.string.no_passphrase),
            ).joinToString(" · "),
            style = MaterialTheme.typography.bodyMedium,
        )
        if (preview.fingerprint.isNotBlank()) {
            Text(preview.fingerprint, style = MaterialTheme.typography.bodySmall, fontFamily = FontFamily.Monospace, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (preview.comment.isNotBlank()) {
            Text(preview.comment, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (preview.encrypted && preview.fingerprint.isBlank()) {
            Text(
                stringResource(R.string.enter_the_passphrase_below_to_import_the_public),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}
