package com.termoso.android.ui.account

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Autorenew
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.DeleteOutline
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
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
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.compose.viewModel
import com.termoso.android.data.AccountManager
import com.termoso.android.data.ReauthCancelled
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.keychain.copyText
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.DeviceKeyCard
import com.termoso.core.KeyItem
import com.termoso.core.SshIdKeyCard
import com.termoso.core.SshIdKeyKind
import com.termoso.core.SshIdView
import com.termoso.core.TermosoApp
import com.termoso.core.sshidHandleValid
import com.termoso.core.sshidProvisionCommand
import com.termoso.core.sshidTypeLabel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.text.DateFormat
import java.util.Date

fun SshIdKeyKind.label(): String = sshidTypeLabel(this)

data class SshIdUiState(
    val loading: Boolean = true,
    val view: SshIdView? = null,
    val working: Boolean = false,
    val error: String? = null,
    /** Keychain security keys that could be published under the handle; `null` while not asked for. */
    val attachable: List<KeyItem>? = null,
)

/**
 * Settings → Account → SSH ID. Every call goes through the Rust façade: the
 * passkeys are generated and kept in the encrypted store, the server and this
 * screen only ever see public keys and fingerprints.
 */
class SshIdViewModel(private val repo: VaultRepository, private val account: AccountManager) : ViewModel() {
    private val _state = MutableStateFlow(SshIdUiState())
    val state: StateFlow<SshIdUiState> = _state.asStateFlow()

    /** Read-only refresh: never prompts, so stale device keys show as "Not published". */
    fun reload() {
        viewModelScope.launch { call(guarded = false) { sshid() } }
    }

    /** Push this device's keys; the server may ask to confirm the password first. */
    suspend fun publish(): Boolean = call { sshidPublish() }

    suspend fun create(handle: String): Boolean = call { sshidCreate(handle) }

    suspend fun rotate(): Boolean = call { sshidRotate() }

    suspend fun removeKey(id: String): Boolean = call { sshidRemoveKey(id) }

    suspend fun delete(): Boolean = call { sshidDelete() }

    /** Publish an existing keychain `sk-*` key under the handle (only its public half leaves the phone). */
    suspend fun attach(keyId: String): Boolean = call { sshidAttachSecurityKey(keyId) }

    /**
     * Security keys from every unlocked vault that are not yet published: the
     * Rust side refuses passphrase-protected handles and duplicates, so they
     * are left out of the picker up front.
     */
    fun loadAttachable() {
        val published = _state.value.view?.keys?.map { it.publicKey.trim() }?.toSet().orEmpty()
        viewModelScope.launch {
            runCatching { repo.read { keys(null) } }
                .onSuccess { all -> _state.update { it.copy(attachable = attachableKeys(all, published)) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun attachableShown() = _state.update { it.copy(attachable = null) }

    private suspend fun call(guarded: Boolean = true, block: TermosoApp.() -> SshIdView): Boolean {
        _state.update { it.copy(working = true, error = null) }
        return runCatching { if (guarded) account.withReauth { repo.read(block) } else repo.read(block) }
            .onSuccess { v -> _state.update { it.copy(loading = false, working = false, view = v) } }
            .onFailure { e ->
                _state.update { it.copy(loading = false, working = false, error = e.takeUnless { it is ReauthCancelled }?.userMessage()) }
            }
            .isSuccess
    }
}

/** Pure filter behind [SshIdViewModel.loadAttachable]. */
fun attachableKeys(all: List<KeyItem>, published: Set<String>): List<KeyItem> =
    all.filter { it.securityKey && !it.unreadable && (!it.encrypted || it.hasPassphrase) && it.publicKey.trim() !in published }

@Composable
fun SshIdScreen(shell: ShellViewModel, account: AccountManager, onBack: () -> Unit, onAddSecurityKey: () -> Unit) {
    val vm: SshIdViewModel = viewModel { SshIdViewModel(shell.repo, account) }
    val state by vm.state.collectAsStateWithLifecycle()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirmRotate by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var removeKey by remember { mutableStateOf<SshIdKeyCard?>(null) }
    var menu by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) { vm.reload() }
    LaunchedEffect(state.error) { state.error?.let { shell.notify(it) } }

    val view = state.view
    val handle = view?.handle

    SubScreen(
        title = "SSH ID",
        onBack = onBack,
        actions = {
            if (handle != null) {
                IconButton(
                    onClick = { scope.launch { if (vm.publish()) shell.notify("Keys are up to date") } },
                    enabled = !state.working,
                ) {
                    Icon(Icons.Filled.Refresh, contentDescription = "Refresh and re-publish")
                }
                Box {
                    IconButton(onClick = { menu = true }, enabled = !state.working) {
                        Icon(Icons.Filled.MoreVert, contentDescription = "More")
                    }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(
                            text = { Text("Rotate this device's keys") },
                            leadingIcon = { Icon(Icons.Filled.Autorenew, contentDescription = null) },
                            onClick = { menu = false; confirmRotate = true },
                        )
                        DropdownMenuItem(
                            text = { Text("Delete SSH ID", color = MaterialTheme.colorScheme.error) },
                            leadingIcon = {
                                Icon(Icons.Filled.DeleteOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                            },
                            onClick = { menu = false; confirmDelete = true },
                        )
                    }
                }
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            when {
                state.loading -> Box(Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                }
                view == null || !view.signedIn -> EmptyState(
                    title = "Sign in to use SSH ID",
                    hint = "SSH ID publishes this phone's public keys under a handle so servers can trust them with one command.",
                )
                handle == null -> SetupSection(
                    working = state.working,
                    onCreate = { h ->
                        scope.launch { if (vm.create(h)) shell.notify("SSH ID @${h.trimStart('@').lowercase()} is ready") }
                    },
                )
                else -> HandleSections(
                    view = view,
                    working = state.working,
                    onCopy = { label, text -> copyText(context, label, text); shell.notify("$label copied") },
                    onRemoveKey = { removeKey = it },
                    onPublish = { scope.launch { if (vm.publish()) shell.notify("Keys published") } },
                    onAddSecurityKey = onAddSecurityKey,
                    onAttachSecurityKey = vm::loadAttachable,
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (confirmRotate) {
        AlertDialog(
            onDismissRequest = { confirmRotate = false },
            title = { Text("Rotate keys?") },
            text = {
                Text(
                    "New Ed25519, ECDSA and RSA passkeys are generated on this phone and published in place of the current ones. " +
                        "Servers that fetched the old keys need to run the provision command again.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmRotate = false
                    scope.launch { if (vm.rotate()) shell.notify("Keys rotated and published") }
                }) { Text("Rotate") }
            },
            dismissButton = { TextButton(onClick = { confirmRotate = false }) { Text("Cancel") } },
        )
    }
    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete SSH ID?") },
            text = {
                Text(
                    "@$handle and every key published under it are removed from the server; the passkeys on this phone are wiped. " +
                        "Identities that log in with SSH ID fall back to their other credentials.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = false
                    scope.launch { if (vm.delete()) shell.notify("SSH ID deleted") }
                }) { Text("Delete", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = false }) { Text("Cancel") } },
        )
    }
    state.attachable?.let { keys ->
        AttachKeyDialog(
            keys = keys,
            onPick = { k ->
                vm.attachableShown()
                scope.launch { if (vm.attach(k.id)) shell.notify("“${k.label}” published under @$handle") }
            },
            onDismiss = vm::attachableShown,
        )
    }
    removeKey?.let { k ->
        AlertDialog(
            onDismissRequest = { removeKey = null },
            title = { Text("Remove key?") },
            text = {
                Text(
                    if (k.hardware) {
                        "The ${k.keyType.label()} security-key entry “${k.label}” is removed from @$handle."
                    } else {
                        "The ${k.keyType.label()} key of “${k.label}” is removed from @$handle. " +
                            "It comes back the next time that device syncs — revoke the device instead to keep it out."
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    val id = k.id
                    removeKey = null
                    scope.launch { if (vm.removeKey(id)) shell.notify("Key removed") }
                }) { Text("Remove", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { removeKey = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun SetupSection(working: Boolean, onCreate: (String) -> Unit) {
    var handle by remember { mutableStateOf("") }
    val valid = sshidHandleValid(handle)
    Spacer(Modifier.height(16.dp))
    Text("Choose your handle", style = MaterialTheme.typography.titleMedium)
    Spacer(Modifier.height(4.dp))
    Text(
        "Your public keys become available at a stable address; on a server run one command to allow this phone in. " +
            "Private keys never leave the phone.",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Spacer(Modifier.height(16.dp))
    OutlinedTextField(
        value = handle,
        onValueChange = { handle = it.trim() },
        label = { Text("Handle") },
        prefix = { Text("@") },
        singleLine = true,
        enabled = !working,
        isError = handle.isNotEmpty() && !valid,
        supportingText = { Text("3–32 characters: lowercase letters, digits, - or _") },
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(Modifier.height(12.dp))
    Button(onClick = { onCreate(handle) }, enabled = valid && !working, modifier = Modifier.fillMaxWidth()) {
        if (working) CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp) else Text("Create SSH ID")
    }
    Spacer(Modifier.height(8.dp))
    Text(
        "Three passkeys (Ed25519, ECDSA, RSA) are generated in the encrypted vault on this phone and only their public halves are published.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun HandleSections(
    view: SshIdView,
    working: Boolean,
    onCopy: (String, String) -> Unit,
    onRemoveKey: (SshIdKeyCard) -> Unit,
    onPublish: () -> Unit,
    onAddSecurityKey: () -> Unit,
    onAttachSecurityKey: () -> Unit,
) {
    val handle = view.handle ?: return
    val url = view.url ?: return
    val command = sshidProvisionCommand(url, null)

    Spacer(Modifier.height(8.dp))
    SectionCard {
        ListRow(
            title = "@$handle",
            subtitle = url,
            leading = { IconTile(Icons.Filled.Fingerprint) },
            trailing = {
                IconButton(onClick = { onCopy("SSH ID URL", url) }) {
                    Icon(Icons.Filled.ContentCopy, contentDescription = "Copy URL")
                }
            },
        )
    }

    SectionLabel("Allow this phone on a server")
    SectionCard {
        Column(Modifier.padding(16.dp)) {
            Text(
                command,
                style = MaterialTheme.typography.bodySmall,
                fontFamily = FontFamily.Monospace,
            )
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                TextButton(onClick = { onCopy("Provision command", command) }) {
                    Icon(Icons.Filled.Terminal, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.size(6.dp))
                    Text("Copy command")
                }
            }
            Text(
                "Appends the published Ed25519 keys to authorized_keys of the user you run it as. " +
                    "Add /all to the URL for every key type, or /ECDSA, /RSA for one.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }

    SectionLabel("This phone")
    SectionCard {
        if (view.deviceKeys.isEmpty()) {
            ListRow(title = "No passkeys yet", subtitle = "Tap refresh to generate and publish them")
        }
        view.deviceKeys.forEachIndexed { i, k ->
            if (i > 0) RowDivider()
            DeviceKeyRow(k, onCopy = { onCopy("${k.keyType.label()} public key", k.publicKey) })
        }
        if (view.deviceKeys.isEmpty() || view.deviceKeys.any { !it.published }) {
            RowDivider()
            Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    "Publishing keys is a security-sensitive change: the server asks you to confirm your password first.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))
                Button(onClick = onPublish, enabled = !working) { Text("Publish keys") }
            }
        }
    }

    val others = view.keys.filter { !it.currentDevice }
    if (others.isNotEmpty()) {
        SectionLabel("Other devices and security keys")
        SectionCard {
            others.forEachIndexed { i, k ->
                if (i > 0) RowDivider()
                PublishedKeyRow(k, enabled = !working, onCopy = { onCopy("${k.keyType.label()} public key", k.publicKey) }, onRemove = { onRemoveKey(k) })
            }
        }
    }

    SectionLabel("Security keys")
    SectionCard {
        ListRow(
            title = "New security key",
            subtitle = "Create a credential on a FIDO2 token (USB or NFC) and publish it",
            leading = { IconTile(Icons.Filled.Usb) },
            modifier = Modifier.clickable(enabled = !working, onClick = onAddSecurityKey),
        )
        RowDivider()
        ListRow(
            title = "Publish a key from the keychain",
            subtitle = "An sk-* key already in a vault on this phone",
            leading = { IconTile(Icons.Filled.VpnKey) },
            modifier = Modifier.clickable(enabled = !working, onClick = onAttachSecurityKey),
        )
    }
    Text(
        "Only the public key and its type go to the server; the credential stays on the token. " +
            "Provision servers with /all or the SK type to accept it.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 4.dp, vertical = 8.dp),
    )
    if (working) {
        Spacer(Modifier.height(16.dp))
        Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
        }
    }
}

@Composable
private fun AttachKeyDialog(keys: List<KeyItem>, onPick: (KeyItem) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Publish a security key") },
        text = {
            if (keys.isEmpty()) {
                Text(
                    "No security key to publish: every sk-* key in the unlocked vaults is already under this handle, " +
                        "or its handle is passphrase-protected without the passphrase remembered.",
                )
            } else {
                Column {
                    keys.forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = k.label,
                            subtitle = "${k.keyType} · ${k.fingerprint}",
                            leading = { IconTile(Icons.Filled.Usb) },
                            modifier = Modifier.clickable { onPick(k) },
                        )
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(if (keys.isEmpty()) "OK" else "Cancel") } },
    )
}

@Composable
private fun DeviceKeyRow(k: DeviceKeyCard, onCopy: () -> Unit) {
    ListRow(
        title = k.keyType.label(),
        subtitle = k.fingerprint,
        leading = { IconTile(Icons.Filled.PhoneAndroid) },
        modifier = Modifier.clickable(onClick = onCopy),
        trailing = {
            Text(
                if (k.published) "Published" else "Not published",
                style = MaterialTheme.typography.bodySmall,
                color = if (k.published) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            )
        },
    )
}

@Composable
private fun PublishedKeyRow(k: SshIdKeyCard, enabled: Boolean, onCopy: () -> Unit, onRemove: () -> Unit) {
    var menu by remember { mutableStateOf(false) }
    val updated = DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(k.updatedAt))
    ListRow(
        title = "${k.keyType.label()} · ${k.label}",
        subtitle = "${k.fingerprint.ifEmpty { "—" }} · $updated",
        leading = { IconTile(if (k.hardware) Icons.Filled.Usb else Icons.Filled.Key) },
        trailing = {
            Box {
                IconButton(onClick = { menu = true }, enabled = enabled) {
                    Icon(Icons.Filled.MoreVert, contentDescription = "Key actions")
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(
                        text = { Text("Copy public key") },
                        leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                        onClick = { menu = false; onCopy() },
                    )
                    DropdownMenuItem(
                        text = { Text("Remove", color = MaterialTheme.colorScheme.error) },
                        leadingIcon = {
                            Icon(Icons.Filled.DeleteOutline, contentDescription = null, tint = MaterialTheme.colorScheme.error)
                        },
                        onClick = { menu = false; onRemove() },
                    )
                }
            }
        },
    )
}
