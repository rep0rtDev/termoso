package com.termoso.android.ui.settings

import android.content.Intent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.Code
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.SyncProblem
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.net.toUri
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.BuildConfig
import com.termoso.android.data.AccountManager
import com.termoso.android.data.AppContainer
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.security.AuthResult
import com.termoso.android.ui.security.authenticateDevice
import com.termoso.android.ui.security.deviceAuthProblem
import com.termoso.android.ui.security.findFragmentActivity
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.android.ui.theme.Emerald
import com.termoso.core.MobileSettings
import com.termoso.core.SyncState
import com.termoso.core.coreVersion
import com.termoso.core.terminalTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private val themes = listOf("system" to "System", "dark" to "Dark", "light" to "Light")

private val lockDelays = listOf(
    0u to "Immediately",
    30u to "After 30 seconds",
    60u to "After 1 minute",
    300u to "After 5 minutes",
    900u to "After 15 minutes",
    3600u to "After 1 hour",
)

private const val SOURCE_URL = "https://github.com/rep0rtDev/termoso"

/** Settings tab: account/sync, appearance, terminal, security (app lock), about. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    shell: ShellViewModel,
    container: AppContainer,
    account: AccountManager,
    onAccount: () -> Unit,
    onSignIn: () -> Unit,
    onLock: () -> Unit,
    onTerminalAppearance: () -> Unit,
) {
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val accountStatus by account.status.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    fun set(transform: (MobileSettings) -> MobileSettings) {
        scope.launch { shell.repo.updateSettings(transform) }
    }
    var themePicker by remember { mutableStateOf(false) }
    var delayPicker by remember { mutableStateOf(false) }
    var privacy by remember { mutableStateOf(false) }
    var licenses by remember { mutableStateOf(false) }
    val appLock by container.appLock.collectAsStateWithLifecycle()
    var lockBusy by remember { mutableStateOf(false) }

    /**
     * Toggle the Keystore-gated master key. Both directions require passing the
     * system prompt first so the vault can never end up behind an unusable lock.
     */
    fun setAppLock(enable: Boolean) {
        if (lockBusy) return
        if (enable) {
            deviceAuthProblem(context)?.let { shell.notify(it); return }
        }
        val activity = context.findFragmentActivity() ?: return
        lockBusy = true
        scope.launch {
            try {
                val title = if (enable) "Turn on app lock" else "Turn off app lock"
                when (val r = authenticateDevice(activity, title, "Confirm it's you")) {
                    AuthResult.Success -> {
                        runCatching { withContext(Dispatchers.IO) { container.setAppLock(enable) } }
                            .onSuccess { shell.notify(if (enable) "App lock is on" else "App lock is off") }
                            .onFailure { shell.notify(it.message ?: "Could not change app lock") }
                    }
                    AuthResult.Cancelled -> {}
                    is AuthResult.Failed -> shell.notify(r.message)
                }
            } finally {
                lockBusy = false
            }
        }
    }

    Scaffold(topBar = { TopAppBar(title = { Text("Settings") }) }) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp),
        ) {
            Spacer(Modifier.height(8.dp))
            SectionCard {
                val card = accountStatus.account
                if (card == null) {
                    ChevronRow(
                        title = "Termoso Cloud",
                        subtitle = "Free forever · E2E-encrypted sync · or your own server",
                        leading = { IconTile(Icons.Filled.Cloud) },
                        modifier = Modifier.clickable(onClick = onSignIn),
                    )
                } else {
                    val sync = accountStatus.sync
                    ChevronRow(
                        title = card.displayName ?: card.email,
                        subtitle = card.serverUrl.removePrefix("https://").removePrefix("http://") + " · " +
                            when (sync.state) {
                                SyncState.IDLE -> if (sync.realtime) "synced · live" else "synced"
                                SyncState.SYNCING -> "syncing…"
                                SyncState.OFFLINE -> "offline"
                                SyncState.ERROR -> "sync failed"
                            },
                        leading = {
                            IconTile(
                                when (sync.state) {
                                    SyncState.OFFLINE -> Icons.Filled.CloudOff
                                    SyncState.ERROR -> Icons.Filled.SyncProblem
                                    else -> Icons.Filled.Cloud
                                },
                                tint = when (sync.state) {
                                    SyncState.ERROR -> MaterialTheme.colorScheme.error
                                    else -> Emerald
                                },
                            )
                        },
                        modifier = Modifier.clickable(onClick = onAccount),
                    )
                }
            }

            SectionLabel("Appearance")
            SectionCard {
                ChevronRow(
                    title = "App theme",
                    badge = themes.firstOrNull { it.first == settings.appTheme }?.second ?: settings.appTheme,
                    modifier = Modifier.clickable { themePicker = true },
                )
                RowDivider()
                ChevronRow(
                    title = "Terminal",
                    subtitle = "${terminalTheme(settings.terminalTheme)?.name ?: settings.terminalTheme} · " +
                        "${settings.terminalFontFamily} ${settings.terminalFontSize}",
                    leading = { IconTile(Icons.Filled.Palette) },
                    modifier = Modifier.clickable(onClick = onTerminalAppearance),
                )
            }

            SectionLabel("Terminal")
            SectionCard {
                SwitchRow(
                    title = "Detect OS",
                    subtitle = "Read the remote OS after connecting to show its icon",
                    checked = settings.detectOs,
                    onCheckedChange = { v -> set { it.copy(detectOs = v) } },
                )
                RowDivider()
                SwitchRow(
                    title = "Post-quantum key exchange",
                    subtitle = "Prefer ML-KEM hybrid KEX when the server supports it",
                    checked = settings.postQuantumKex,
                    onCheckedChange = { v -> set { it.copy(postQuantumKex = v) } },
                )
                RowDivider()
                SwitchRow(
                    title = "Keep screen on",
                    subtitle = "While a terminal is in the foreground",
                    checked = settings.keepScreenOn,
                    onCheckedChange = { v -> set { it.copy(keepScreenOn = v) } },
                )
                RowDivider()
                SwitchRow(
                    title = "Haptic feedback",
                    checked = settings.hapticFeedback,
                    onCheckedChange = { v -> set { it.copy(hapticFeedback = v) } },
                )
                RowDivider()
                SwitchRow(
                    title = "Terminal bell",
                    subtitle = "Vibrate on BEL",
                    checked = settings.terminalBell,
                    onCheckedChange = { v -> set { it.copy(terminalBell = v) } },
                )
            }

            SectionLabel("Security")
            SectionCard {
                SwitchRow(
                    title = "App lock",
                    subtitle = if (appLock) {
                        "Fingerprint or screen lock required to open the vault"
                    } else {
                        "Protect the vault with your fingerprint or screen lock"
                    },
                    checked = appLock,
                    enabled = !lockBusy,
                    onCheckedChange = ::setAppLock,
                )
                RowDivider()
                SwitchRow(
                    title = "Lock when in background",
                    subtitle = "Ask again after leaving the app; sessions keep running",
                    checked = settings.lockOnBackground,
                    enabled = appLock,
                    onCheckedChange = { v -> set { it.copy(lockOnBackground = v) } },
                )
                RowDivider()
                ChevronRow(
                    title = "Lock after",
                    badge = lockDelays.firstOrNull { it.first == settings.lockAfterSeconds }?.second ?: "${settings.lockAfterSeconds} s",
                    modifier = Modifier.clickable(enabled = appLock && settings.lockOnBackground) { delayPicker = true },
                )
                RowDivider()
                if (appLock) {
                    ChevronRow(
                        title = "Lock now",
                        subtitle = "Cover the app until you authenticate; sessions stay connected",
                        leading = { IconTile(Icons.Filled.Fingerprint) },
                        modifier = Modifier.clickable(onClick = container::gate),
                    )
                    RowDivider()
                }
                ChevronRow(
                    title = "Lock vault now",
                    subtitle = "Disconnects every session and closes the encrypted database",
                    leading = { IconTile(Icons.Filled.Lock) },
                    modifier = Modifier.clickable(onClick = onLock),
                )
            }

            SectionLabel("About")
            SectionCard {
                ListRow(
                    title = "Termoso for Android",
                    subtitle = "${BuildConfig.VERSION_NAME} · core ${coreVersion()}",
                    leading = { IconTile(Icons.Filled.Info) },
                )
                RowDivider()
                ChevronRow(
                    title = "Privacy",
                    subtitle = "No telemetry, no analytics, no accounts required",
                    leading = { IconTile(Icons.Filled.Shield) },
                    modifier = Modifier.clickable { privacy = true },
                )
                RowDivider()
                ChevronRow(
                    title = "Source code",
                    subtitle = "AGPL-licensed, on GitHub",
                    leading = { IconTile(Icons.Filled.Code) },
                    modifier = Modifier.clickable {
                        runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, SOURCE_URL.toUri())) }
                            .onFailure { shell.notify(SOURCE_URL) }
                    },
                )
                RowDivider()
                ChevronRow(title = "Open-source licenses", modifier = Modifier.clickable { licenses = true })
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (themePicker) {
        RadioDialog("App theme", themes, settings.appTheme, onPick = { set { s -> s.copy(appTheme = it) } }) { themePicker = false }
    }
    if (delayPicker) {
        RadioDialog("Lock after", lockDelays, settings.lockAfterSeconds, onPick = { set { s -> s.copy(lockAfterSeconds = it) } }) { delayPicker = false }
    }
    if (privacy) {
        AlertDialog(
            onDismissRequest = { privacy = false },
            title = { Text("Privacy") },
            text = {
                Text(
                    "Termoso reports to you, not on you.\n\n" +
                        "• No telemetry, crash reporting or analytics SDKs — the app contains none.\n" +
                        "• Hosts, keys, passwords and settings live in an encrypted database on this device; " +
                        "the key is wrapped by Android Keystore and never written in plain text.\n" +
                        "• Private keys leave the device only when you export them yourself.\n" +
                        "• When you sign in to a Termoso server (Termoso Cloud or your own), only end-to-end " +
                        "encrypted data is synced — the server cannot read your vault.\n" +
                        "• No ads, no plans, no upsells. Free software, forever.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            },
            confirmButton = { TextButton(onClick = { privacy = false }) { Text("Close") } },
        )
    }
    if (licenses) {
        AlertDialog(
            onDismissRequest = { licenses = false },
            title = { Text("Open-source licenses") },
            text = {
                Text(
                    "Termoso — AGPL-3.0.\n\n" +
                        "Bundled terminal fonts:\n" +
                        "• JetBrains Mono — SIL Open Font License 1.1\n" +
                        "• Fira Code — SIL Open Font License 1.1\n" +
                        "• Source Code Pro — SIL Open Font License 1.1\n" +
                        "• Ubuntu Mono — Ubuntu Font Licence 1.0\n\n" +
                        "Full texts ship in apps/android/licenses in the source tree. Rust and Android library " +
                        "notices are listed in the repository.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            },
            confirmButton = { TextButton(onClick = { licenses = false }) { Text("Close") } },
        )
    }
}

@Composable
private fun <T> RadioDialog(title: String, options: List<Pair<T, String>>, selected: T, onPick: (T) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                options.forEach { (id, label) ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .clickable { onPick(id); onDismiss() }
                            .padding(vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = selected == id, onClick = null)
                        Text(label)
                    }
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
    )
}
