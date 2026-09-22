package com.termoso.android.data

import android.content.Context
import android.net.Uri
import android.os.SystemClock
import android.view.KeyEvent
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import com.termoso.android.saf.FilesIntegration
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
    data class Open(
        val repo: VaultRepository,
        val sessions: SessionManager,
        val sftp: SftpManager,
        val forwards: ForwardManager,
        val account: AccountManager,
        val keepAlive: KeepAlive,
        val presence: PresenceManager,
        val ai: AiManager,
    ) : VaultState
}

/**
 * Process-wide dependencies (no DI framework): the profile directory, the
 * Keystore-wrapped master key and the open [TermosoApp] handle.
 */
class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    val profileDir: File = File(context.noBackupFilesDir, "profile")
    val masterKeys = MasterKeyStore(context)

    /** FIDO2 tokens on USB/NFC; process-wide, started on first use by the UI. */
    val fido2 = Fido2Manager(appContext)

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

    /**
     * A team invitation link the app was opened with (`termoso://invite/<token>`),
     * held until the vault is open and the shell can show the join dialog.
     */
    private val _pendingInvite = MutableStateFlow<String?>(null)
    val pendingInvite: StateFlow<String?> = _pendingInvite.asStateFlow()

    fun offerInvite(link: String) {
        _pendingInvite.value = link
    }

    fun consumeInvite() {
        _pendingInvite.value = null
    }

    /**
     * A `termoso://join/…` multiplayer link the app was opened with. Held as one
     * opaque string (its fragment is the session secret) until a signed-in
     * shell can hand it to Rust; never logged.
     */
    private val _pendingJoin = MutableStateFlow<String?>(null)
    val pendingJoin: StateFlow<String?> = _pendingJoin.asStateFlow()

    fun offerJoin(link: String) {
        _pendingJoin.value = link
    }

    fun consumeJoin() {
        _pendingJoin.value = null
    }

    /**
     * Files shared into the app (`ACTION_SEND[_MULTIPLE]`), waiting for the
     * terminal to offer sending them to the active session. Content URIs stay
     * on the Kotlin side; Rust only ever sees app-owned scratch copies.
     */
    private val _pendingShare = MutableStateFlow<List<Uri>?>(null)
    val pendingShare: StateFlow<List<Uri>?> = _pendingShare.asStateFlow()

    fun offerShare(uris: List<Uri>) {
        if (uris.isNotEmpty()) _pendingShare.value = uris
    }

    fun consumeShare() {
        _pendingShare.value = null
    }

    /**
     * Hardware-key hook installed by the terminal while it is on screen
     * (volume-key bindings, Ctrl(+Shift) hotkeys). The activity consults it
     * before normal dispatch; `null` or a `false` return keeps the system
     * behaviour, so volume keys stay volume keys everywhere else.
     */
    @Volatile
    var hardwareKeyHook: ((KeyEvent) -> Boolean)? = null

    /** SFTP hosts as a storage root for the system Files UI and other apps. */
    val files = FilesIntegration(appContext)

    init {
        ProcessLifecycleOwner.get().lifecycle.addObserver(
            object : DefaultLifecycleObserver {
                override fun onStop(owner: LifecycleOwner) {
                    backgroundedAt = SystemClock.elapsedRealtime()
                }

                override fun onStart(owner: LifecycleOwner) {
                    if (backgroundLockDue()) _gated.value = true
                    backgroundedAt = 0L
                }
            },
        )
        files.watch(this)
    }

    /**
     * Whether the in-use app lock would cover the UI right now: the vault is
     * open, "lock when in background" is on and the app has been away longer
     * than the configured delay. Raised as [gated] when the UI returns; other
     * entry points (the documents provider) consult it directly.
     */
    fun backgroundLockDue(): Boolean {
        val open = _vault.value as? VaultState.Open ?: return false
        val settings = open.repo.settings.value
        if (!settings.lockOnBackground || !masterKeys.authRequired() || backgroundedAt == 0L) return false
        val away = (SystemClock.elapsedRealtime() - backgroundedAt) / 1000
        return away >= settings.lockAfterSeconds.toLong()
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
        }.also {
            val sessions = SessionManager(it, File(appContext.filesDir, "home"))
            val sftp = SftpManager(appContext, it)
            val forwards = ForwardManager(it)
            val account = AccountManager(it, onSignOut = sessions::endLive)
            _vault.value = VaultState.Open(
                repo = it,
                sessions = sessions,
                sftp = sftp,
                forwards = forwards,
                account = account,
                keepAlive = KeepAlive(appContext, sessions, sftp, forwards),
                presence = PresenceManager(it, account),
                ai = AiManager(it, account),
            )
        }
    }

    /**
     * Open the vault only if that needs no one present: a profile exists and
     * its master key is not bound to device authentication. `null` otherwise —
     * never creates a profile and never bypasses the app lock.
     */
    suspend fun unlockSilently(): VaultRepository? {
        (_vault.value as? VaultState.Open)?.let { return it.repo }
        if (!hasProfile() || masterKeys.authRequired()) return null
        return unlock()
    }

    /** Disconnect every terminal, close the store and drop the handle. */
    suspend fun lockVault() = lock.withLock {
        val open = _vault.value as? VaultState.Open ?: return@withLock
        _vault.value = VaultState.Locked
        _gated.value = false
        open.sessions.closeAll()
        open.sftp.closeAll()
        open.forwards.closeAll()
        open.keepAlive.close()
        open.presence.close()
        open.ai.close()
        open.account.close()
        fido2.close()
        withContext(Dispatchers.IO) {
            open.repo.app.forgetCachedPassphrases()
            open.repo.app.close()
        }
    }
}
