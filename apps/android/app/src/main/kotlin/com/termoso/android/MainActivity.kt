package com.termoso.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import com.termoso.android.ui.TermosoRoot
import com.termoso.android.ui.theme.TermosoTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val container = (application as TermosoApplication).container
        setContent { App(container) }
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
        TermosoRoot(container = container, vault = vault)
    }
}
