package com.termoso.android.saf

import android.content.Context
import android.provider.DocumentsContract
import androidx.core.content.edit
import com.termoso.android.BuildConfig
import com.termoso.android.data.AppContainer
import com.termoso.android.data.VaultState
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

/**
 * The "show SFTP hosts in Files" switch and the roots-changed signal behind
 * it. The switch lives in plain preferences, not the encrypted settings: the
 * system asks for roots while the vault is still locked, and whether Termoso
 * appears in the picker at all is a device-level choice, not vault data.
 */
class FilesIntegration(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val resolver = context.applicationContext.contentResolver

    private val _enabled = MutableStateFlow(prefs.getBoolean(KEY_ENABLED, true))
    val enabled: StateFlow<Boolean> = _enabled.asStateFlow()

    fun setEnabled(value: Boolean) {
        prefs.edit { putBoolean(KEY_ENABLED, value) }
        _enabled.value = value
        notifyRootsChanged()
    }

    /** Tell every Files UI to re-query our roots. */
    fun notifyRootsChanged() {
        resolver.notifyChange(DocumentsContract.buildRootsUri(AUTHORITY), null)
    }

    /**
     * Roots follow the vault (locked ↔ open) and the host list inside it, so a
     * host renamed or added in the app shows up in the picker without a restart.
     */
    @OptIn(ExperimentalCoroutinesApi::class)
    fun watch(container: AppContainer, scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)) {
        scope.launch {
            container.vault
                .flatMapLatest { state ->
                    when (state) {
                        is VaultState.Open -> state.repo.revision.map { true to it }
                        VaultState.Locked -> flowOf(false to 0)
                    }
                }
                .combine(container.appLock) { vault, lock -> Triple(vault.first, vault.second, lock) }
                .distinctUntilChanged()
                .collectLatest { notifyRootsChanged() }
        }
    }

    companion object {
        const val AUTHORITY: String = BuildConfig.APPLICATION_ID + ".documents"
        private const val PREFS = "files_integration"
        private const val KEY_ENABLED = "enabled"
    }
}
