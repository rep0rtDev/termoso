package com.termoso.android.ui.keychain

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Security
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.IdentityItem
import com.termoso.core.KeyItem

/** Keys and identities of the selected vault; `+` offers Generate / Paste key / FIDO2 / New identity. */
@Composable
fun KeychainScreen(
    shell: ShellViewModel,
    onBack: () -> Unit,
    onGenerate: () -> Unit,
    onImport: () -> Unit,
    onFido2: () -> Unit,
    onFido2Load: () -> Unit,
    onOpenKey: (String) -> Unit,
    onNewIdentity: () -> Unit,
    onOpenIdentity: (String) -> Unit,
) {
    val vaultId by shell.selectedVaultId.collectAsStateWithLifecycle()
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    var keys by remember { mutableStateOf<List<KeyItem>>(emptyList()) }
    var identities by remember { mutableStateOf<List<IdentityItem>>(emptyList()) }
    var menu by remember { mutableStateOf(false) }
    LaunchedEffect(vaultId, revision) {
        runCatching { shell.repo.read { keys(vaultId) to identities(vaultId) } }
            .onSuccess { (k, i) -> keys = k; identities = i }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen(
        stringResource(R.string.keychain),
        onBack,
        floating = {
            Box {
                FloatingActionButton(onClick = { menu = true }) { Icon(Icons.Filled.Add, contentDescription = stringResource(R.string.add)) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.generate_key)) },
                        leadingIcon = { Icon(Icons.Filled.VpnKey, contentDescription = null) },
                        onClick = { menu = false; onGenerate() },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.paste_or_import_key)) },
                        leadingIcon = { Icon(Icons.Filled.ContentPaste, contentDescription = null) },
                        onClick = { menu = false; onImport() },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.new_fido2_key)) },
                        leadingIcon = { Icon(Icons.Filled.Security, contentDescription = null) },
                        onClick = { menu = false; onFido2() },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.load_from_security_key)) },
                        leadingIcon = { Icon(Icons.Filled.Usb, contentDescription = null) },
                        onClick = { menu = false; onFido2Load() },
                    )
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.new_identity)) },
                        leadingIcon = { Icon(Icons.Filled.Person, contentDescription = null) },
                        onClick = { menu = false; onNewIdentity() },
                    )
                }
            }
        },
    ) { padding ->
        if (keys.isEmpty() && identities.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = stringResource(R.string.add_credentials),
                    hint = stringResource(R.string.generate_an_ssh_key_or_paste_one_from),
                    icon = Icons.Filled.Key,
                    action = {
                        Column(horizontalAlignment = Alignment.CenterHorizontally) {
                            Button(onClick = onGenerate) { Text(stringResource(R.string.generate_key)) }
                            TextButton(onClick = onImport) { Text(stringResource(R.string.paste_or_import_key)) }
                        }
                    },
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 96.dp)) {
            if (keys.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.keys)) }
                item {
                    SectionCard {
                        keys.forEachIndexed { i, k ->
                            if (i > 0) RowDivider()
                            ListRow(
                                title = k.label,
                                subtitle = keySubtitle(k),
                                leading = { IconTile(Icons.Filled.Key) },
                                titleColor = if (k.unreadable) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                                modifier = Modifier.clickable { onOpenKey(k.id) },
                            )
                        }
                    }
                }
            }
            if (identities.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.identities)) }
                item {
                    SectionCard {
                        identities.forEachIndexed { i, id ->
                            if (i > 0) RowDivider()
                            ListRow(
                                title = id.label,
                                subtitle = identitySubtitle(id),
                                leading = { IconTile(Icons.Filled.Person) },
                                modifier = Modifier.clickable { onOpenIdentity(id.id) },
                            )
                        }
                    }
                }
            }
        }
    }
}

fun keySubtitle(k: KeyItem): String = listOfNotNull(
    keyTypeLabel(k.keyType, k.bits),
    k.fingerprint.takeIf { it.isNotBlank() }?.removePrefix("SHA256:")?.take(12),
    when {
        k.unreadable -> str(R.string.unreadable)
        k.encrypted && k.hasPassphrase -> str(R.string.passphrase_saved)
        k.encrypted -> str(R.string.passphrase)
        else -> null
    },
    if (k.hasCertificate) str(R.string.certificate_2) else null,
    if (k.usedBy > 0u) str(R.string.used_by, k.usedBy) else null,
).joinToString(" · ")

fun identitySubtitle(id: IdentityItem): String = listOfNotNull(
    id.username.takeIf { it.isNotBlank() },
    if (id.hasPassword) str(R.string.password_2) else null,
    id.sshKeyLabel,
    if (id.hasCertificate) str(R.string.certificate_2) else null,
).joinToString(" · ").ifBlank { str(R.string.no_credentials) }

fun keyTypeLabel(keyType: String, bits: UInt): String {
    val t = keyType.lowercase()
    val base = when {
        t.contains("ed25519") -> "Ed25519"
        t.contains("rsa") -> str(R.string.rsa, bits)
        t.contains("ecdsa") || t.contains("nistp") -> str(R.string.ecdsa, bits)
        t.contains("dsa") -> str(R.string.dsa, bits)
        t.isBlank() -> str(R.string.unknown)
        else -> "${keyType.uppercase()} $bits".trim()
    }
    return if (t.startsWith("sk-")) str(R.string.fido2, base) else base
}
