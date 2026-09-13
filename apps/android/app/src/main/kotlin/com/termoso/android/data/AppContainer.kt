package com.termoso.android.data

import android.content.Context
import com.termoso.core.TermosoApp
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Vault lifecycle: locked until the master key is unwrapped and the store opened. */
sealed interface VaultState {
    data object Locked : VaultState
    data class Open(val repo: VaultRepository, val sessions: SessionManager) : VaultState
}

/**
 * Process-wide dependencies (no DI framework): the profile directory, the
 * Keystore-wrapped master key and the open [TermosoApp] handle.
 */
class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    val profileDir: File = File(context.noBackupFilesDir, "profile")
    val masterKeys = MasterKeyStore(context)

    private val _vault = MutableStateFlow<VaultState>(VaultState.Locked)
    val vault: StateFlow<VaultState> = _vault.asStateFlow()
    private val lock = Mutex()

    /** True when a profile exists on disk (returning user), false on first launch. */
    fun hasProfile(): Boolean = masterKeys.exists()

    /** Open the encrypted store, creating the master key on first launch. */
    suspend fun unlock(): VaultRepository = lock.withLock {
        (_vault.value as? VaultState.Open)?.let { return it.repo }
        withContext(Dispatchers.IO) {
            val key = if (masterKeys.exists()) masterKeys.unwrap() else masterKeys.create()
            val app = try {
                TermosoApp.open(profileDir.absolutePath, key)
            } finally {
                key.fill(0)
            }
            VaultRepository(app)
        }.also { _vault.value = VaultState.Open(it, SessionManager(appContext, it)) }
    }

    /** Disconnect every terminal, close the store and drop the handle. */
    suspend fun lockVault() = lock.withLock {
        val open = _vault.value as? VaultState.Open ?: return@withLock
        _vault.value = VaultState.Locked
        open.sessions.closeAll()
        withContext(Dispatchers.IO) { open.repo.app.close() }
    }
}
