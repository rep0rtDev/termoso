package com.termoso.android.data

import com.termoso.core.MobileSettings
import com.termoso.core.TermosoApp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    private val settingsLock = Mutex()

    suspend fun <T> read(block: TermosoApp.() -> T): T = withContext(Dispatchers.IO) { app.block() }

    suspend fun <T> write(block: TermosoApp.() -> T): T =
        read(block).also { bump() }

    /** Data changed outside [write] (sync pull, sign-in/out): make lists reload. */
    fun bump() {
        _revision.update { it + 1 }
    }

    /** Settings changed on the Rust side (e.g. by the account runtime): re-read them. */
    suspend fun reloadSettings() {
        _settings.value = read { settings() }
    }

    /**
     * Apply [transform] to the current settings and persist the result. The
     * new value is published before the write so toggles react on the spot;
     * updates run one at a time so two quick taps compose instead of the
     * later one overwriting the earlier with a stale base. What the store
     * hands back afterwards (clamped, sanitized) is what stays published.
     */
    suspend fun updateSettings(transform: (MobileSettings) -> MobileSettings) {
        settingsLock.withLock {
            val next = transform(_settings.value)
            if (next == _settings.value) return
            _settings.value = next
            try {
                read { saveSettings(next) }
            } finally {
                _settings.value = read { settings() }
            }
        }
    }
}
