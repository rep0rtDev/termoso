package com.termoso.android.ui.account

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
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Logout
import androidx.compose.material.icons.filled.PhoneAndroid
import androidx.compose.material.icons.filled.Security
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
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.data.ReauthCancelled
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.components.UserAvatar
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.terminal.aiProviderLabel
import com.termoso.android.ui.terminal.aiRemainingToday
import com.termoso.android.ui.theme.Danger
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
    onSshId: () -> Unit,
    onSecurityKeys: () -> Unit,
) {
    val status by account.status.collectAsStateWithLifecycle()
    val hidden by shell.presence.hidden.collectAsStateWithLifecycle()
    val ai by shell.ai.status.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var devices by remember { mutableStateOf<List<DeviceCard>?>(null) }
    var devicesError by remember { mutableStateOf<String?>(null) }
    var syncing by remember { mutableStateOf(false) }
    var confirmSignOut by remember { mutableStateOf(false) }
    var confirmCredentials by remember { mutableStateOf<Boolean?>(null) }
    var credentialsBusy by remember { mutableStateOf(false) }
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    var revoking by remember { mutableStateOf<DeviceCard?>(null) }

    suspend fun loadDevices() {
        runCatching { account.devices() }
            .onSuccess { devices = it; devicesError = null }
            .onFailure { devicesError = it.userMessage() }
    }

    LaunchedEffect(status.account?.userId) {
        if (status.account != null) {
            loadDevices()
            shell.presence.loadHidden()
            shell.ai.refresh()
        }
    }

    val card = status.account
    SubScreen(title = stringResource(R.string.account), onBack = onBack) { padding ->
        if (card == null) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                Text(stringResource(R.string.not_signed_in), color = MaterialTheme.colorScheme.onSurfaceVariant)
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
                    UserAvatar(
                        repo = shell.repo,
                        userId = card.userId,
                        tag = card.avatar,
                        name = card.displayName ?: card.email,
                        size = 48,
                        shape = RoundedCornerShape(12.dp),
                        textStyle = MaterialTheme.typography.titleLarge,
                    )
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
                            serverLabel(card.serverUrl) + if (card.isAdmin) stringResource(R.string.sep_admin) else "",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }

            SectionLabel(stringResource(R.string.sync))
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
                                        r.lastError?.let { str(R.string.sync_finished_with_errors, it) }
                                            ?: str(R.string.synced_pushed_pulled, r.pushed, r.pulled),
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
                            Text(stringResource(R.string.sync_now))
                        }
                    }
                }
                sync.lastError?.let {
                    RowDivider()
                    ListRow(title = stringResource(R.string.last_error), subtitle = it, titleColor = MaterialTheme.colorScheme.error)
                }
                RowDivider()
                ListRow(
                    title = stringResource(R.string.end_to_end_encrypted),
                    subtitle = stringResource(R.string.the_server_stores_only_ciphertext_keys_stay_on),
                    leading = { IconTile(Icons.Filled.Lock) },
                )
                RowDivider()
                SwitchRow(
                    title = stringResource(R.string.sync_keys_and_identities),
                    subtitle = if (settings.syncCredentials) {
                        stringResource(R.string.identities_keys_and_certificates_of_your_personal_vault)
                    } else {
                        stringResource(R.string.keys_of_your_personal_vault_stay_on_this) +
                            (if (status.localCredentials > 0u) stringResource(R.string.here, status.localCredentials) else "") +
                            stringResource(R.string.hosts_and_snippets_still_sync_they_are_deleted)
                    },
                    checked = settings.syncCredentials,
                    enabled = !credentialsBusy,
                    onCheckedChange = { on -> confirmCredentials = on },
                )
            }

            SectionLabel(stringResource(R.string.teams))
            SectionCard {
                val teamVaults = status.vaults.count { it.kind == VaultKind.TEAM }
                ChevronRow(
                    title = stringResource(R.string.teams),
                    subtitle = when (teamVaults) {
                        0 -> stringResource(R.string.share_vaults_with_colleagues_create_a_team_or)
                        1 -> stringResource(R.string.s_1_team_vault_on_this_device)
                        else -> stringResource(R.string.team_vaults_on_this_device, teamVaults)
                    },
                    leading = { IconTile(Icons.Filled.Group) },
                    modifier = Modifier.clickable(onClick = onTeams),
                )
            }

            SectionLabel(stringResource(R.string.security))
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.ssh_id),
                    subtitle = stringResource(R.string.publish_this_phones_public_keys_under_a_handle),
                    leading = { IconTile(Icons.Filled.Fingerprint) },
                    modifier = Modifier.clickable(onClick = onSshId),
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.security_keys),
                    subtitle = stringResource(R.string.fido2_keys_over_usb_or_nfc_as_the),
                    leading = { IconTile(Icons.Filled.Security) },
                    modifier = Modifier.clickable(onClick = onSecurityKeys),
                )
                RowDivider()
                SwitchRow(
                    title = stringResource(R.string.show_me_as_connected),
                    subtitle = stringResource(R.string.teammates_see_which_team_vault_host_you_are),
                    checked = hidden == false,
                    enabled = hidden != null,
                    onCheckedChange = { on ->
                        scope.launch {
                            runCatching { shell.presence.setHidden(!on) }.onFailure { shell.notify(it.userMessage()) }
                        }
                    },
                )
            }

            SectionLabel(stringResource(R.string.ai_command_suggestions))
            SectionCard {
                val s = ai
                when {
                    s == null -> ListRow(title = stringResource(R.string.checking_the_server))
                    !s.available -> ListRow(
                        title = stringResource(R.string.not_offered_by_this_server),
                        subtitle = stringResource(R.string.the_operator_can_point_termoso_ai_at_a),
                    )
                    else -> {
                        SwitchRow(
                            title = stringResource(R.string.suggest_commands_from_a_description),
                            subtitle = stringResource(R.string.of_left_today, aiRemainingToday(s), s.dailyQuota) +
                                aiProviderLabel(s) +
                                (if (s.confidential) stringResource(R.string.sep_confidential_compute) else ""),
                            checked = s.enabled,
                            onCheckedChange = { on ->
                                scope.launch {
                                    runCatching { shell.ai.setEnabled(on) }.onFailure { shell.notify(it.userMessage()) }
                                }
                            },
                        )
                        RowDivider()
                        Text(
                            stringResource(R.string.sends_only_your_request_text_and_an_os) +
                                if (s.confidential) stringResource(R.string.confidential_compute_means_the_operator_cannot_read_requests) else "",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
                        )
                    }
                }
            }

            SectionLabel(stringResource(R.string.vaults_on_this_device))
            SectionCard {
                status.vaults.forEachIndexed { i, v ->
                    if (i > 0) RowDivider()
                    ListRow(
                        title = v.name,
                        subtitle = listOfNotNull(
                            when (v.kind) {
                                VaultKind.LOCAL -> stringResource(R.string.local_only)
                                VaultKind.PERSONAL -> stringResource(R.string.personal_synced)
                                VaultKind.TEAM -> stringResource(R.string.team)
                            },
                            when (v.access) {
                                VaultAccess.MANAGE -> null
                                VaultAccess.EDIT -> stringResource(R.string.can_edit)
                                VaultAccess.VIEW -> stringResource(R.string.view_only)
                            },
                            if (v.locked) stringResource(R.string.key_pending) else null,
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

            SectionLabel(stringResource(R.string.devices))
            SectionCard {
                when {
                    devices == null && devicesError == null -> Box(Modifier.fillMaxWidth().padding(20.dp), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                    }
                    devicesError != null -> ListRow(
                        title = stringResource(R.string.could_not_load_devices),
                        subtitle = devicesError,
                        titleColor = MaterialTheme.colorScheme.error,
                    ) {
                        TextButton(onClick = { devices = null; devicesError = null; scope.launch { loadDevices() } }) { Text(stringResource(R.string.retry)) }
                    }
                    else -> devices.orEmpty().forEachIndexed { i, d ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = d.name + if (d.current) stringResource(R.string.this_device) else "",
                            subtitle = stringResource(R.string.last_seen, d.platform, d.appVersion, relative(d.lastSeenAt)),
                            leading = { IconTile(platformIcon(d.platform)) },
                        ) {
                            if (!d.current) {
                                IconButton(onClick = { revoking = d }) {
                                    Icon(Icons.Filled.Delete, contentDescription = stringResource(R.string.sign_out_this_device), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                                }
                            }
                        }
                    }
                }
            }

            Spacer(Modifier.height(16.dp))
            SectionCard {
                ListRow(
                    title = stringResource(R.string.sign_out),
                    subtitle = stringResource(R.string.synced_vaults_are_removed_from_this_device_local),
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
            title = { Text(stringResource(R.string.sign_out_2)) },
            text = {
                Column {
                    Text(
                        stringResource(R.string.this_device_is_removed_from_your_account_and),
                    )
                    if (status.localCredentials > 0u) {
                        Spacer(Modifier.height(12.dp))
                        Text(
                            stringResource(R.string.sync_of_keys_and_identities_is_off_of, status.localCredentials),
                            color = Warning,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmSignOut = false
                    scope.launch {
                        runCatching { account.signOut() }
                            .onSuccess { shell.notify(str(R.string.signed_out)); onSignedOut() }
                            .onFailure { shell.notify(it.userMessage()) }
                    }
                }) { Text(stringResource(R.string.sign_out), color = Danger) }
            },
            dismissButton = { TextButton(onClick = { confirmSignOut = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    confirmCredentials?.let { on ->
        AlertDialog(
            onDismissRequest = { confirmCredentials = null },
            title = { Text(if (on) stringResource(R.string.sync_keys_and_identities_2) else stringResource(R.string.keep_keys_and_identities_on_this_phone)) },
            text = {
                Text(
                    if (on) {
                        stringResource(R.string.identities_keys_and_certificates_of_your_personal_vault_2)
                    } else {
                        stringResource(R.string.identities_keys_and_certificates_of_your_personal_vault_3)
                    },
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    confirmCredentials = null
                    credentialsBusy = true
                    scope.launch {
                        runCatching { account.setCredentialSync(on) }
                            .onSuccess {
                                shell.notify(if (on) str(R.string.keys_and_identities_are_synced_again) else str(R.string.keys_and_identities_now_stay_on_this_phone))
                            }
                            .onFailure { shell.notify(it.userMessage()) }
                        credentialsBusy = false
                    }
                }) { Text(if (on) stringResource(R.string.sync) else stringResource(R.string.keep_local), color = if (on) MaterialTheme.colorScheme.primary else Danger) }
            },
            dismissButton = { TextButton(onClick = { confirmCredentials = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    revoking?.let { d ->
        AlertDialog(
            onDismissRequest = { revoking = null },
            title = { Text(stringResource(R.string.sign_out_3, d.name)) },
            text = { Text(stringResource(R.string.that_device_loses_access_to_your_account_immediately)) },
            confirmButton = {
                TextButton(onClick = {
                    revoking = null
                    scope.launch {
                        runCatching { account.revokeDevice(d.id) }
                            .onSuccess { shell.notify(str(R.string.signed_out_2, d.name)); loadDevices() }
                            .onFailure { if (it !is ReauthCancelled) shell.notify(it.userMessage()) }
                    }
                }) { Text(stringResource(R.string.sign_out), color = Danger) }
            },
            dismissButton = { TextButton(onClick = { revoking = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun SyncTile(sync: SyncStatus) {
    when (sync.state) {
        SyncState.IDLE -> IconTile(Icons.Filled.Sync, tint = MaterialTheme.colorScheme.primary)
        SyncState.SYNCING -> IconTile(Icons.Filled.Sync)
        SyncState.OFFLINE -> IconTile(Icons.Filled.CloudOff, tint = Warning)
        SyncState.ERROR -> IconTile(Icons.Filled.SyncProblem, tint = Danger)
    }
}

private fun syncTitle(s: SyncStatus): String = when (s.state) {
    SyncState.IDLE -> if (s.realtime) str(R.string.up_to_date_live) else str(R.string.up_to_date)
    SyncState.SYNCING -> str(R.string.syncing)
    SyncState.OFFLINE -> str(R.string.offline)
    SyncState.ERROR -> str(R.string.sync_failed)
}

private fun syncSubtitle(s: SyncStatus): String {
    val last = s.lastSyncAt?.let { str(R.string.last_sync, relative(it)) } ?: str(R.string.not_synced_yet)
    val counts = if (s.pushed > 0u || s.pulled > 0u || s.conflicts > 0u) {
        " · ↑${s.pushed} ↓${s.pulled}" + if (s.conflicts > 0u) str(R.string.sep_conflicts, s.conflicts.toLong()) else ""
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
        secs < 45 -> str(R.string.just_now)
        secs < 3600 -> str(R.string.min_ago, secs / 60)
        secs < 86_400 -> str(R.string.h_ago, secs / 3600)
        else -> str(R.string.d_ago, secs / 86_400)
    }
}
