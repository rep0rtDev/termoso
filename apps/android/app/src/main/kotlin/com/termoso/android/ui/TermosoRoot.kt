package com.termoso.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.shell.MainShell
import com.termoso.android.ui.welcome.WelcomeScreen
import kotlinx.coroutines.launch

/**
 * Entry composable. Opens the vault on launch (the Keystore-wrapped key needs no
 * user input yet; PIN/biometric gating lands with Settings → Security), shows the
 * welcome screen once, then the tabbed shell. An explicit lock from Settings keeps
 * the vault closed until the user taps Unlock.
 */
@Composable
fun TermosoRoot(container: AppContainer, vault: VaultState) {
    var error by remember { mutableStateOf<String?>(null) }
    var attempt by remember { mutableIntStateOf(0) }
    var manualLock by remember { mutableStateOf(false) }
    var cloudNotice by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(attempt, manualLock) {
        if (vault is VaultState.Locked && !manualLock) {
            runCatching { container.unlock() }.onFailure { error = it.userMessage() }
        }
    }
    when (vault) {
        is VaultState.Locked -> LockedScreen(
            error = error,
            manual = manualLock,
            onRetry = { error = null; manualLock = false; attempt++ },
        )
        is VaultState.Open -> {
            val settings by vault.repo.settings.collectAsStateWithLifecycle()
            if (!settings.welcomeSeen) {
                WelcomeScreen(
                    onCloud = { cloudNotice = true },
                    onContinueOffline = {
                        scope.launch { vault.repo.updateSettings { it.copy(welcomeSeen = true) } }
                    },
                )
            } else {
                val session = remember(vault.repo) { SessionStoreOwner() }
                DisposableEffect(session) { onDispose { session.viewModelStore.clear() } }
                CompositionLocalProvider(LocalViewModelStoreOwner provides session) {
                    MainShell(
                        repo = vault.repo,
                        onCloud = { cloudNotice = true },
                        onLock = {
                            manualLock = true
                            scope.launch { container.lockVault() }
                        },
                    )
                }
            }
        }
    }
    if (cloudNotice) {
        CloudNoticeDialog(onDismiss = { cloudNotice = false })
    }
}

/** ViewModel scope bound to one opened vault, so nothing outlives a lock. */
private class SessionStoreOwner : ViewModelStoreOwner {
    override val viewModelStore = ViewModelStore()
}

@Composable
private fun LockedScreen(error: String?, manual: Boolean, onRetry: () -> Unit) {
    Scaffold { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when {
                manual -> {
                    IconTile(Icons.Filled.Lock, size = 64, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.height(16.dp))
                    Text("Vault locked", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "The encrypted database is closed. Unlock to continue.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = onRetry) { Text("Unlock") }
                }
                error == null -> CircularProgressIndicator()
                else -> {
                    Text("Could not open the vault", style = MaterialTheme.typography.titleMedium)
                    Text(error, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
                    Spacer(Modifier.height(16.dp))
                    Button(onClick = onRetry) { Text("Retry") }
                }
            }
        }
    }
}

@Composable
private fun CloudNoticeDialog(onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Termoso Cloud on Android") },
        text = {
            Text(
                "Account sign-in and encrypted sync arrive in the next Android update. " +
                    "Termoso Cloud stays completely free — no limits, no plans, no strings attached. " +
                    "Everything you create now stays in the encrypted vault on this device and will sync once you sign in.",
            )
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("OK") } },
    )
}
