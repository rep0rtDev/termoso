package com.termoso.android.ui.settings

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
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Lock
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
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.BuildConfig
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SwitchRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.MobileSettings
import com.termoso.core.coreVersion
import kotlinx.coroutines.launch

private val themes = listOf("system" to "System", "dark" to "Dark", "light" to "Light")

/** Settings tab: account (later), appearance, terminal, security, about. Themes/fonts/PIN expand in A5. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(shell: ShellViewModel, onCloud: () -> Unit, onLock: () -> Unit) {
    val settings by shell.repo.settings.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    fun set(transform: (MobileSettings) -> MobileSettings) {
        scope.launch { shell.repo.updateSettings(transform) }
    }
    var themePicker by remember { mutableStateOf(false) }

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
                ChevronRow(
                    title = "Termoso Cloud",
                    subtitle = "Free forever · E2E-encrypted sync · sign-in coming to Android",
                    leading = { IconTile(Icons.Filled.Cloud) },
                    modifier = Modifier.clickable(onClick = onCloud),
                )
            }

            SectionLabel("Appearance")
            SectionCard {
                ChevronRow(
                    title = "Theme",
                    badge = themes.firstOrNull { it.first == settings.appTheme }?.second ?: settings.appTheme,
                    modifier = Modifier.clickable { themePicker = true },
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
            }

            SectionLabel("Security")
            SectionCard {
                ChevronRow(
                    title = "Lock vault now",
                    subtitle = "Closes the encrypted database until the app restarts",
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
                Column(Modifier.padding(16.dp)) {
                    Text(
                        "Free, open source, no telemetry. Nothing leaves this device unless you sign in to a " +
                            "Termoso server — and then only end-to-end encrypted data.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (themePicker) {
        AlertDialog(
            onDismissRequest = { themePicker = false },
            title = { Text("Theme") },
            text = {
                Column {
                    themes.forEach { (id, label) ->
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .clickable { set { it.copy(appTheme = id) }; themePicker = false }
                                .padding(vertical = 4.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            RadioButton(selected = settings.appTheme == id, onClick = null)
                            Text(label)
                        }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { themePicker = false }) { Text("Close") } },
        )
    }
}
