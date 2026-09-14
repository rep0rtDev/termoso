package com.termoso.android.data

import android.content.Context
import android.os.SystemClock
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
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
    data class Open(val repo: VaultRepository, val sessions: SessionManager, val account: AccountManager) : VaultState
}

/**
 * Process-wide dependencies (no DI framework): the profile directory, the
 * Keystore-wrapped master key and the open [TermosoApp] handle.
 */
class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    val profileDir: File = File(context.noBackupFilesDir, "profile")
    val masterKeys = MasterKeyStore(context)

    /** Whether opening the vault needs device authentication (auth-bound Keystore wrapper). */
    private val _appLock = MutableStateFlow(masterKeys.authRequired())
    val appLock: StateFlow<Boolean> = _appLock.asStateFlow()

    private val _vault = MutableStateFlow<VaultState>(VaultState.Locked)
    val vault: StateFlow<VaultState> = _vault.asStateFlow()
    private val lock = Mutex()

    /**
     * In-use app lock: the vault stays open (SSH sessions keep running under the
     * foreground service) but the UI is covered until the user authenticates.
     * Raised when the app returns from the background after the configured delay.
     */
    private val _gated = MutableStateFlow(false)
    val gated: StateFlow<Boolean> = _gated.asStateFlow()
    private var backgroundedAt = 0L

    init {
        ProcessLifecycleOwner.get().lifecycle.addObserver(
            object : DefaultLifecycleObserver {
                override fun onStop(owner: LifecycleOwner) {
                    backgroundedAt = SystemClock.elapsedRealtime()
                }

                override fun onStart(owner: LifecycleOwner) {
                    val open = _vault.value as? VaultState.Open ?: return
                    val settings = open.repo.settings.value
                    if (!settings.lockOnBackground || !masterKeys.authRequired() || backgroundedAt == 0L) return
                    val away = (SystemClock.elapsedRealtime() - backgroundedAt) / 1000
                    if (away >= settings.lockAfterSeconds.toLong()) _gated.value = true
                }
            },
        )
    }

    fun gate() {
        if (_vault.value is VaultState.Open && masterKeys.authRequired()) _gated.value = true
    }

    /** Blocking Keystore work; call off the main thread after the user authenticated. */
    fun setAppLock(enabled: Boolean) {
        try {
            masterKeys.setAuthRequired(enabled)
        } finally {
            _appLock.value = masterKeys.authRequired()
        }
    }

    fun ungate() {
        _gated.value = false
    }

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
        }.also { _vault.value = VaultState.Open(it, SessionManager(appContext, it), AccountManager(it)) }
    }

    /** Disconnect every terminal, close the store and drop the handle. */
    suspend fun lockVault() = lock.withLock {
        val open = _vault.value as? VaultState.Open ?: return@withLock
        _vault.value = VaultState.Locked
        _gated.value = false
        open.sessions.closeAll()
        open.account.close()
        withContext(Dispatchers.IO) { open.repo.app.close() }
    }
}
