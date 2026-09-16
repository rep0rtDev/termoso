package com.termoso.android

import android.annotation.SuppressLint
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
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

    /**
     * Terminal-installed hook first (volume bindings, app hotkeys); everything else as usual.
     * ComponentActivity marks this override as restricted although it is the only place that
     * sees volume keys and chords before the focused view consumes them.
     */
    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val hook = (application as TermosoApplication).container.hardwareKeyHook
        if (hook != null && hook(event)) return true
        return super.dispatchKeyEvent(event)
    }

    private fun handleLink(intent: Intent?, container: AppContainer) {
        when (intent?.action) {
            Intent.ACTION_SEND, Intent.ACTION_SEND_MULTIPLE -> {
                container.offerShare(sharedUris(intent))
                // Consumed: a rotation must not re-offer the same files.
                intent.action = Intent.ACTION_MAIN
                return
            }
            Intent.ACTION_VIEW -> Unit
            else -> return
        }
        val link = intent.dataString ?: return
        when (classifyLink(link)) {
            LinkKind.Invite -> container.offerInvite(link)
            LinkKind.Join -> container.offerJoin(link)
            LinkKind.Other -> Unit
        }
    }
}

/** Content URIs of a share intent: `EXTRA_STREAM` first, then the clip items (keyboard image paste, some galleries). */
internal fun sharedUris(intent: Intent): List<Uri> {
    val out = LinkedHashSet<Uri>()
    if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
        val list = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)
        }
        list?.forEach { out += it }
    } else {
        val one = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(Intent.EXTRA_STREAM)
        }
        one?.let { out += it }
    }
    intent.clipData?.let { clip ->
        for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let { out += it }
    }
    return out.filter { it.scheme == "content" }
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
