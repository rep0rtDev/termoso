package com.termoso.android.ui.team

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.AccountManager
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.TeamMemberCard
import com.termoso.core.VaultAccess
import com.termoso.core.VaultMemberCard
import kotlinx.coroutines.launch

/**
 * One team vault: who can open it and how. Managers grant, change and revoke
 * access; every grant seals the key on this phone, every revoke rotates it.
 */
@Composable
fun TeamVaultScreen(
    shell: ShellViewModel,
    account: AccountManager,
    vaultId: String,
    onBack: () -> Unit,
) {
    val status by account.status.collectAsStateWithLifecycle()
    val vault = status.vaults.firstOrNull { it.id == vaultId }
    val scope = rememberCoroutineScope()
    var members by remember { mutableStateOf<List<VaultMemberCard>?>(null) }
    var teamMembers by remember { mutableStateOf<List<TeamMemberCard>>(emptyList()) }
    var reload by remember { mutableIntStateOf(0) }
    var menu by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    var rotating by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    var adding by remember { mutableStateOf(false) }
    var memberMenu by remember { mutableStateOf<VaultMemberCard?>(null) }
    var revoking by remember { mutableStateOf<VaultMemberCard?>(null) }
    val canManage = vault?.access == VaultAccess.MANAGE && !vault.locked

    LaunchedEffect(vaultId, reload, vault?.teamId) {
        val teamId = vault?.teamId ?: return@LaunchedEffect
        runCatching {
            shell.repo.read { teamVaultMembers(vaultId) to teamMembers(teamId) }
        }.onSuccess { (m, t) -> members = m; teamMembers = t }
            .onFailure { shell.notify(it.userMessage()) }
    }

    fun mutate(done: String? = null, block: suspend () -> Unit) {
        scope.launch {
            runCatching { block() }
                .onSuccess { reload++; done?.let(shell::notify) }
                .onFailure { shell.notify(it.userMessage()) }
        }
    }

    SubScreen(
        title = vault?.name ?: "Vault",
        onBack = onBack,
        actions = {
            if (canManage) {
                Box {
                    IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "More") }
                    DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                        DropdownMenuItem(text = { Text("Rename vault") }, onClick = { menu = false; renaming = true })
                        DropdownMenuItem(text = { Text("Rotate key") }, onClick = { menu = false; rotating = true })
                        DropdownMenuItem(
                            text = { Text("Delete vault", color = MaterialTheme.colorScheme.error) },
                            onClick = { menu = false; deleting = true },
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
            Spacer(Modifier.height(8.dp))
            if (vault == null) {
                Box(Modifier.fillMaxWidth().padding(32.dp), contentAlignment = Alignment.Center) {
                    Text("This vault is no longer on this device.", color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                return@SubScreen
            }
            SectionCard {
                ListRow(
                    title = vault.name,
                    subtitle = if (vault.locked) "Key not received yet — ask a manager to grant it" else "You ${vault.access.label()}",
                    leading = { IconTile(if (vault.locked) Icons.Filled.Lock else Icons.Filled.LockOpen) },
                )
            }

            SectionLabel("Access")
            SectionCard {
                val list = members
                when {
                    list == null -> Box(Modifier.fillMaxWidth().padding(24.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                    }
                    else -> list.forEachIndexed { i, m ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = (m.displayName ?: m.email) + if (m.me) " (you)" else "",
                            subtitle = m.access.label() + if (m.pending) " · waiting for key" else "",
                            leading = { IconTile(Icons.Filled.Person) },
                            trailing = when {
                                m.pending && canManage -> {
                                    {
                                        TextButton(onClick = { mutate { shell.repo.read { setTeamVaultAccess(vaultId, m.userId, m.access) } } }) {
                                            Text("Grant")
                                        }
                                    }
                                }
                                canManage && !m.me -> {
                                    { IconButton(onClick = { memberMenu = m }) { Icon(Icons.Filled.MoreVert, contentDescription = "Manage") } }
                                }
                                else -> null
                            },
                        )
                    }
                }
                if (canManage) {
                    RowDivider()
                    ListRow(
                        title = "Add member",
                        titleColor = MaterialTheme.colorScheme.primary,
                        leading = { IconTile(Icons.Filled.PersonAdd, tint = MaterialTheme.colorScheme.primary) },
                        modifier = Modifier.clickable { adding = true },
                    )
                }
            }
            if (canManage) {
                Text(
                    "Granting seals the vault key to the member's account key on this phone. Revoking rotates the key " +
                        "and re-seals it for everyone who stays; the server only ever stores ciphertext.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (renaming && vault != null) {
        NameDialog(
            title = "Rename vault",
            label = "Vault name",
            initial = vault.name,
            onDismiss = { renaming = false },
            onConfirm = { renaming = false; mutate { shell.repo.write { renameTeamVault(vaultId, it) } } },
        )
    }
    if (rotating) {
        ConfirmDialog(
            title = "Rotate the vault key?",
            text = "A new key is generated on this phone, everything in the vault is re-encrypted and the key is " +
                "re-sealed for every current member. Use it if you suspect a copy leaked.",
            confirm = "Rotate",
            onConfirm = { rotating = false; mutate("Vault key rotated") { shell.repo.write { rotateTeamVaultKey(vaultId) } } },
            onDismiss = { rotating = false },
        )
    }
    if (deleting && vault != null) {
        ConfirmDialog(
            title = "Delete \"${vault.name}\"?",
            text = "Its hosts, keys and snippets are deleted for every member. This cannot be undone.",
            confirm = "Delete",
            onConfirm = {
                deleting = false
                scope.launch {
                    runCatching { shell.repo.write { deleteTeamVault(vaultId) } }
                        .onSuccess { shell.notify("Vault deleted"); onBack() }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { deleting = false },
        )
    }
    if (adding) {
        val present = members?.map { it.userId }?.toSet() ?: emptySet()
        AddVaultMemberDialog(
            candidates = teamMembers.filter { !it.me && it.userId !in present },
            onDismiss = { adding = false },
            onAdd = { userId, access -> adding = false; mutate { shell.repo.read { setTeamVaultAccess(vaultId, userId, access) } } },
        )
    }
    memberMenu?.let { m ->
        AlertDialog(
            onDismissRequest = { memberMenu = null },
            title = { Text(m.displayName ?: m.email) },
            text = {
                Column {
                    listOf(VaultAccess.VIEW, VaultAccess.EDIT, VaultAccess.MANAGE).forEach { a ->
                        val current = m.access == a
                        ListRow(
                            title = a.label() + if (current) " · current" else "",
                            subtitle = a.hint(),
                            titleColor = if (current) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.clickable(enabled = !current) {
                                memberMenu = null
                                mutate { shell.repo.read { setTeamVaultAccess(vaultId, m.userId, a) } }
                            },
                        )
                    }
                    ListRow(
                        title = "Revoke access",
                        titleColor = MaterialTheme.colorScheme.error,
                        modifier = Modifier.clickable { memberMenu = null; revoking = m },
                    )
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { memberMenu = null }) { Text("Cancel") } },
        )
    }
    revoking?.let { m ->
        ConfirmDialog(
            title = "Revoke access for ${m.displayName ?: m.email}?",
            text = "The vault key is rotated on this phone and re-sealed for the remaining members.",
            confirm = "Revoke",
            onConfirm = { revoking = null; mutate { shell.repo.write { removeTeamVaultAccess(vaultId, m.userId) } } },
            onDismiss = { revoking = null },
        )
    }
}

@Composable
private fun AddVaultMemberDialog(
    candidates: List<TeamMemberCard>,
    onDismiss: () -> Unit,
    onAdd: (userId: String, access: VaultAccess) -> Unit,
) {
    var picked by remember { mutableStateOf<TeamMemberCard?>(null) }
    var access by remember { mutableStateOf(VaultAccess.VIEW) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Add member") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                if (candidates.isEmpty()) {
                    Text(
                        "Everyone in the team already has access. Invite more people from the team screen.",
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                candidates.forEach { m ->
                    val on = picked?.userId == m.userId
                    ListRow(
                        title = m.displayName ?: m.email,
                        subtitle = if (m.displayName != null) m.email else null,
                        leading = { IconTile(Icons.Filled.Person, selected = on) },
                        modifier = Modifier.clickable { picked = m },
                    )
                }
                if (candidates.isNotEmpty()) {
                    Spacer(Modifier.height(8.dp))
                    AccessMenu(current = access, allowNone = false, onPick = { it?.let { a -> access = a } }) { open ->
                        ListRow(title = "Access", subtitle = access.hint(), modifier = Modifier.clickable(onClick = open)) {
                            Text(access.label(), color = MaterialTheme.colorScheme.primary)
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { picked?.let { onAdd(it.userId, access) } }, enabled = picked != null) { Text("Add") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}
