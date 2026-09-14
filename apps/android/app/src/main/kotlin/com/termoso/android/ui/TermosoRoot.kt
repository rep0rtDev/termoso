package com.termoso.android.ui

import android.security.keystore.UserNotAuthenticatedException
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import com.termoso.android.data.userMessage
import com.termoso.android.ui.account.AuthMode
import com.termoso.android.ui.account.SignInScreen
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.security.AuthResult
import com.termoso.android.ui.security.authenticateDevice
import com.termoso.android.ui.security.findFragmentActivity
import com.termoso.android.ui.shell.MainShell
import com.termoso.android.ui.welcome.WelcomeScreen
import kotlinx.coroutines.launch

/**
 * Entry composable. Opens the vault on launch, shows the welcome screen once,
 * then the tabbed shell. With app lock on, the Keystore refuses to unwrap the
 * master key until the user passes the system prompt; returning from the
 * background after the configured delay covers the shell with the same prompt
 * while sessions keep running. An explicit lock from Settings closes the store
 * until the user taps Unlock.
 */
@Composable
fun TermosoRoot(container: AppContainer, vault: VaultState) {
    val context = LocalContext.current
    var error by remember { mutableStateOf<String?>(null) }
    var attempt by remember { mutableIntStateOf(0) }
    var manualLock by remember { mutableStateOf(false) }
    var needAuth by remember { mutableStateOf(false) }
    var welcomeAuth by remember { mutableStateOf<AuthMode?>(null) }
    val gated by container.gated.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    suspend fun prompt(): Boolean {
        val activity = context.findFragmentActivity() ?: return false
        return when (val r = authenticateDevice(activity, "Unlock Termoso", "Use your fingerprint or screen lock")) {
            AuthResult.Success -> true
            AuthResult.Cancelled -> false
            is AuthResult.Failed -> {
                error = r.message
                false
            }
        }
    }

    LaunchedEffect(attempt, manualLock) {
        if (vault !is VaultState.Locked || manualLock) return@LaunchedEffect
        if (needAuth && !prompt()) return@LaunchedEffect
        runCatching { container.unlock() }
            .onSuccess { needAuth = false }
            .onFailure {
                if (it is UserNotAuthenticatedException) {
                    needAuth = true
                    if (prompt()) attempt++
                } else {
                    error = it.userMessage()
                }
            }
    }
    LaunchedEffect(gated) {
        if (gated && prompt()) container.ungate()
    }

    when (vault) {
        is VaultState.Locked -> LockedScreen(
            error = error,
            manual = manualLock || needAuth,
            authRequired = container.masterKeys.authRequired(),
            onRetry = { error = null; manualLock = false; attempt++ },
        )
        is VaultState.Open -> {
            val settings by vault.repo.settings.collectAsStateWithLifecycle()
            if (gated) {
                LockedScreen(error = error, manual = true, authRequired = true, onRetry = {
                    error = null
                    scope.launch { if (prompt()) container.ungate() }
                })
            } else {
                val session = remember(vault.repo) { SessionStoreOwner() }
                DisposableEffect(session) { onDispose { session.viewModelStore.clear() } }
                val restoring by vault.account.restoring.collectAsStateWithLifecycle()
                val accountStatus by vault.account.status.collectAsStateWithLifecycle()
                fun finishWelcome() {
                    welcomeAuth = null
                    scope.launch { vault.repo.updateSettings { it.copy(welcomeSeen = true) } }
                }
                CompositionLocalProvider(LocalViewModelStoreOwner provides session) {
                    when {
                        // A returning user who is still signed in never sees the welcome again.
                        !settings.welcomeSeen && restoring -> LockedScreen(error = null, manual = false, authRequired = false, onRetry = {})
                        !settings.welcomeSeen && accountStatus.account != null -> LaunchedEffect(Unit) { finishWelcome() }
                        !settings.welcomeSeen && welcomeAuth != null -> SignInScreen(
                            account = vault.account,
                            mode = welcomeAuth ?: AuthMode.SignIn,
                            onBack = { welcomeAuth = null },
                            onDone = ::finishWelcome,
                        )
                        !settings.welcomeSeen -> WelcomeScreen(
                            onCreateAccount = { welcomeAuth = AuthMode.Register },
                            onSignIn = { welcomeAuth = AuthMode.SignIn },
                            onContinueOffline = ::finishWelcome,
                        )
                        else -> MainShell(
                            container = container,
                            repo = vault.repo,
                            sessions = vault.sessions,
                            sftp = vault.sftp,
                            account = vault.account,
                            onLock = {
                                manualLock = true
                                scope.launch { container.lockVault() }
                            },
                        )
                    }
                }
            }
        }
    }
}

/** ViewModel scope bound to one opened vault, so nothing outlives a lock. */
private class SessionStoreOwner : ViewModelStoreOwner {
    override val viewModelStore = ViewModelStore()
}

@Composable
private fun LockedScreen(error: String?, manual: Boolean, authRequired: Boolean, onRetry: () -> Unit) {
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
                    Text("Termoso is locked", style = MaterialTheme.typography.titleMedium)
                    Text(
                        error ?: if (authRequired) {
                            "Unlock with your fingerprint or screen lock to continue."
                        } else {
                            "The encrypted vault is closed. Unlock to continue."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
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

