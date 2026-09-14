package com.termoso.android.ui.account

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.Computer
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Logout
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material.icons.filled.SyncProblem
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.AccountManager
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.theme.Danger
import com.termoso.android.ui.theme.Emerald
import com.termoso.android.ui.theme.Warning
import com.termoso.core.DeviceCard
import com.termoso.core.SyncState
import com.termoso.core.SyncStatus
import com.termoso.core.VaultAccess
import com.termoso.core.VaultKind
import java.time.Instant
import java.time.OffsetDateTime
import java.time.format.DateTimeParseException
import kotlinx.coroutines.launch

/** Settings → Account: identity, sync status + Sync now, vaults on this device, devices, sign out. */
@Composable
fun AccountScreen(
    shell: ShellViewModel,
    account: AccountManager,
    onBack: () -> Unit,
    onSignedOut: () -> Unit,
    onTeams: () -> Unit,
) {
    val status by account.status.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var devices by remember { mutableStateOf<List<DeviceCard>?>(null) }
    var devicesError by remember { mutableStateOf<String?>(null) }
    var syncing by remember { mutableStateOf(false) }
    var confirmSignOut by remember { mutableStateOf(false) }
    var revoking by remember { mutableStateOf<DeviceCard?>(null) }

    suspend fun loadDevices() {
        runCatching { account.devices() }
            .onSuccess { devices = it; devicesError = null }
            .onFailure { devicesError = it.userMessage() }
    }

    LaunchedEffect(status.account?.userId) {
        if (status.account != null) loadDevices()
    }

    val card = status.account
    SubScreen(title = "Account", onBack = onBack) { padding ->
        if (card == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text("Not signed in.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            return@SubScreen
        }
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            SectionCard {
                Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically) {
                    Avatar(card.displayName ?: card.email)
                    Spacer(Modifier.width(16.dp))
                    Column(Modifier.weight(1f)) {
                        Text(
                            card.displayName ?: card.email,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                        )
                        if (card.displayName != null) {
                            Text(card.email, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Text(
                            serverLabel(card.serverUrl) + if (card.isAdmin) " · admin" else "",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            SectionLabel("Sync")
            SectionCard {
                val sync = status.sync
                ListRow(
                    title = syncTitle(sync),
                    subtitle = syncSubtitle(sync),
                    leading = { SyncTile(sync) },
                ) {
                    Button(
                        onClick = {
                            if (syncing) return@Button
                            syncing = true
                            scope.launch {
                                try {
                                    val r = account.syncNow()
                                    shell.notify(
                                        r.lastError?.let { "Sync finished with errors: $it" }
                                            ?: "Synced · ${r.pushed} pushed, ${r.pulled} pulled",
                                    )
                                } catch (e: Exception) {
                                    shell.notify(e.userMessage())
                                } finally {
                                    syncing = false
                                }
                            }
                        },
                        enabled = !syncing && sync.state != SyncState.SYNCING,
                    ) {
                        if (syncing || sync.state == SyncState.SYNCING) {
                            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp, color = MaterialTheme.colorScheme.onPrimary)
                        } else {
                            Text("Sync now")
                        }
                    }
                }
                sync.lastError?.let {
                    RowDivider()
                    ListRow(title = "Last error", subtitle = it, titleColor = MaterialTheme.colorScheme.error)
                }
                RowDivider()
                ListRow(
                    title = "End-to-end encrypted",
                    subtitle = "The server stores only ciphertext; keys stay on your devices",
                    leading = { IconTile(Icons.Filled.Lock) },
                )
            }

            SectionLabel("Teams")
            SectionCard {
                val teamVaults = status.vaults.count { it.kind == VaultKind.TEAM }
                ChevronRow(
                    title = "Teams",
                    subtitle = when (teamVaults) {
                        0 -> "Share vaults with colleagues — create a team or join by invitation"
                        1 -> "1 team vault on this device"
                        else -> "$teamVaults team vaults on this device"
                    },
                    leading = { IconTile(Icons.Filled.Group) },
                    modifier = Modifier.clickable(onClick = onTeams),
                )
            }

            SectionLabel("Vaults on this device")
            SectionCard {
                status.vaults.forEachIndexed { i, v ->
                    if (i > 0) RowDivider()
                    ListRow(
                        title = v.name,
                        subtitle = listOfNotNull(
                            when (v.kind) {
                                VaultKind.LOCAL -> "Local only"
                                VaultKind.PERSONAL -> "Personal · synced"
                                VaultKind.TEAM -> "Team"
                            },
                            when (v.access) {
                                VaultAccess.MANAGE -> null
                                VaultAccess.EDIT -> "can edit"
                                VaultAccess.VIEW -> "view only"
                            },
                            if (v.locked) "key pending" else null,
                        ).joinToString(" · "),
                        leading = {
                            IconTile(
                                when (v.kind) {
                                    VaultKind.LOCAL -> Icons.Filled.PhoneAndroid
                                    VaultKind.PERSONAL -> Icons.Filled.Cloud
                                    VaultKind.TEAM -> Icons.Filled.Group
                                },
                            )
                        },
                    )
                }
            }

            SectionLabel("Devices")
            SectionCard {
                when {
                    devices == null && devicesError == null -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                    devicesError != null -> ListRow(
                        title = "Could not load devices",
                        subtitle = devicesError,
                        titleColor = MaterialTheme.colorScheme.error,
                    ) {
                        TextButton(onClick = { devices = null; devicesError = null; scope.launch { loadDevices() } }) { Text("Retry") }
                    }
                    else -> devices.orEmpty().forEachIndexed { i, d ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = d.name + if (d.current) " (this device)" else "",
                            subtitle = "${d.platform} · ${d.appVersion} · last seen ${relative(d.lastSeenAt)}",
                            leading = { IconTile(platformIcon(d.platform)) },
                        ) {
                            if (!d.current) {
                                IconButton(onClick = { revoking = d }) {
                                    Icon(Icons.Filled.Delete, contentDescription = "Sign out this device", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                    }
                }
            }

            Spacer(Modifier.height(16.dp))
            SectionCard {
                ListRow(
                    title = "Sign out",
                    subtitle = "Synced vaults are removed from this device; local vault stays",
                    titleColor = Danger,
                    leading = { IconTile(Icons.Filled.Logout, tint = Danger) },
                    modifier = Modifier.clickable { confirmSignOut = true },
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (confirmSignOut) {
        AlertDialog(
            onDismissRequest = { confirmSignOut = false },
            title = { Text("Sign out?") },
            text = {
                Text(
                    "This device is removed from your account and the synced vaults are deleted from this phone. " +
                        "Your local vault and its hosts stay. You can sign in again any time.",
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmSignOut = false
                    scope.launch {
                        runCatching { account.signOut() }
                            .onSuccess { shell.notify("Signed out"); onSignedOut() }
                            .onFailure { shell.notify(it.userMessage()) }
                    }
                }) { Text("Sign out", color = Danger) }
            },
            dismissButton = { TextButton(onClick = { confirmSignOut = false }) { Text("Cancel") } },
        )
    }
    revoking?.let { d ->
        AlertDialog(
            onDismissRequest = { revoking = null },
            title = { Text("Sign out ${d.name}?") },
            text = { Text("That device loses access to your account immediately and has to sign in again.") },
            confirmButton = {
                TextButton(onClick = {
                    revoking = null
                    scope.launch {
                        runCatching { account.revokeDevice(d.id) }
                            .onSuccess { shell.notify("${d.name} signed out"); loadDevices() }
                            .onFailure { shell.notify(it.userMessage()) }
                    }
                }) { Text("Sign out", color = Danger) }
            },
            dismissButton = { TextButton(onClick = { revoking = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun Avatar(name: String) {
    Box(
        Modifier
            .size(48.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(Emerald),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            name.trim().firstOrNull()?.uppercaseChar()?.toString() ?: "?",
            color = Color.White,
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
        )
    }
}

@Composable
private fun SyncTile(sync: SyncStatus) {
    when (sync.state) {
        SyncState.IDLE -> IconTile(Icons.Filled.Sync, tint = Emerald)
        SyncState.SYNCING -> IconTile(Icons.Filled.Sync)
        SyncState.OFFLINE -> IconTile(Icons.Filled.CloudOff, tint = Warning)
        SyncState.ERROR -> IconTile(Icons.Filled.SyncProblem, tint = Danger)
    }
}

private fun syncTitle(s: SyncStatus): String = when (s.state) {
    SyncState.IDLE -> if (s.realtime) "Up to date · live" else "Up to date"
    SyncState.SYNCING -> "Syncing…"
    SyncState.OFFLINE -> "Offline"
    SyncState.ERROR -> "Sync failed"
}

private fun syncSubtitle(s: SyncStatus): String {
    val last = s.lastSyncAt?.let { "Last sync ${relative(it)}" } ?: "Not synced yet"
    val counts = if (s.pushed > 0u || s.pulled > 0u || s.conflicts > 0u) {
        " · ↑${s.pushed} ↓${s.pulled}" + if (s.conflicts > 0u) " · ${s.conflicts} conflicts" else ""
    } else {
        ""
    }
    return last + counts
}

private fun serverLabel(url: String): String = url.removePrefix("https://").removePrefix("http://").trimEnd('/')

private fun platformIcon(platform: String): ImageVector = when (platform.lowercase()) {
    "android", "ios" -> Icons.Filled.PhoneAndroid
    "web" -> Icons.Filled.Cloud
    else -> Icons.Filled.Computer
}

/** RFC 3339 → "just now" / "5 min ago" / "3 h ago" / "2 d ago". */
fun relative(iso: String): String {
    val at = try {
        OffsetDateTime.parse(iso).toInstant()
    } catch (_: DateTimeParseException) {
        return iso
    }
    val secs = Instant.now().epochSecond - at.epochSecond
    return when {
        secs < 45 -> "just now"
        secs < 3600 -> "${secs / 60} min ago"
        secs < 86_400 -> "${secs / 3600} h ago"
        else -> "${secs / 86_400} d ago"
    }
}
