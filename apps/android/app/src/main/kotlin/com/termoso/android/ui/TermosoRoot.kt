package com.termoso.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import com.termoso.core.coreVersion

/**
 * Entry composable. Opens the vault on launch (the Keystore-wrapped key needs no
 * user input yet; PIN/biometric gating lands with Settings → Security).
 */
@Composable
fun TermosoRoot(container: AppContainer, vault: VaultState) {
    var error by remember { mutableStateOf<String?>(null) }
    var attempt by remember { mutableStateOf(0) }
    LaunchedEffect(attempt) {
        if (vault is VaultState.Locked) {
            runCatching { container.unlock() }.onFailure { error = it.message ?: it.toString() }
        }
    }
    Scaffold { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when (vault) {
                is VaultState.Locked -> {
                    if (error == null) {
                        CircularProgressIndicator()
                    } else {
                        Text("Could not open the vault", style = MaterialTheme.typography.titleMedium)
                        Text(error!!, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
                        Button(onClick = { error = null; attempt++ }) { Text("Retry") }
                    }
                }
                is VaultState.Open -> {
                    val hosts = remember(vault) { runCatching { vault.app.hosts(null).size }.getOrDefault(0) }
                    Text("Termoso", style = MaterialTheme.typography.titleLarge)
                    Text("core ${coreVersion()} · $hosts hosts", style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}
