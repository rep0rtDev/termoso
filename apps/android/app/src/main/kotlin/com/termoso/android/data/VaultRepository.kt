package com.termoso.android.data

import com.termoso.core.MobileSettings
import com.termoso.core.TermosoApp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.withContext

/**
 * The open store. Every call into Rust goes through [read] / [write] so it runs
 * off the main thread; [write] also bumps [revision], which list screens observe
 * to reload after a mutation anywhere in the app.
 */
class VaultRepository(val app: TermosoApp) {
    private val _revision = MutableStateFlow(0)
    val revision: StateFlow<Int> = _revision.asStateFlow()

    private val _settings = MutableStateFlow(app.settings())
    val settings: StateFlow<MobileSettings> = _settings.asStateFlow()

    suspend fun <T> read(block: TermosoApp.() -> T): T = withContext(Dispatchers.IO) { app.block() }

    suspend fun <T> write(block: TermosoApp.() -> T): T =
        read(block).also { bump() }

    /** Data changed outside [write] (sync pull, sign-in/out): make lists reload. */
    fun bump() {
        _revision.update { it + 1 }
    }

    suspend fun updateSettings(transform: (MobileSettings) -> MobileSettings) {
        val next = transform(_settings.value)
        read { saveSettings(next) }
        _settings.value = read { settings() }
    }
}
