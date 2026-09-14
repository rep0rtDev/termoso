package com.termoso.android

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import com.termoso.android.ui.TermosoRoot
import com.termoso.android.ui.keychain.LocalFido2
import com.termoso.android.ui.theme.TermosoTheme
import kotlinx.coroutines.launch

/** [FragmentActivity] rather than ComponentActivity because BiometricPrompt hosts a fragment. */
class MainActivity : FragmentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val container = (application as TermosoApplication).container
        // With app lock on, keep terminal contents out of Recents thumbnails and screen capture.
        lifecycleScope.launch {
            container.appLock.collect { secure ->
                if (secure) {
                    window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                } else {
                    window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) setRecentsScreenshotEnabled(!secure)
            }
        }
        handleLink(intent, container)
        setContent { App(container) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleLink(intent, (application as TermosoApplication).container)
    }

    private fun handleLink(intent: Intent?, container: AppContainer) {
        val link = intent?.takeIf { it.action == Intent.ACTION_VIEW }?.dataString ?: return
        when (classifyLink(link)) {
            LinkKind.Invite -> container.offerInvite(link)
            LinkKind.Join -> container.offerJoin(link)
            LinkKind.Other -> Unit
        }
    }
}

@Composable
private fun App(container: AppContainer) {
    val vault by container.vault.collectAsStateWithLifecycle()
    var theme by rememberSaveable { mutableStateOf("system") }
    val openTheme = (vault as? VaultState.Open)
        ?.repo?.settings?.collectAsStateWithLifecycle()?.value?.appTheme
    LaunchedEffect(openTheme) { if (openTheme != null) theme = openTheme }
    TermosoTheme(appTheme = theme) {
        CompositionLocalProvider(LocalFido2 provides container.fido2) {
            TermosoRoot(container = container, vault = vault)
        }
    }
}
