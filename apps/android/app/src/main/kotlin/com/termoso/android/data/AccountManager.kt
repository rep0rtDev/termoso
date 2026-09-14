package com.termoso.android.data

import android.os.Build
import com.termoso.core.AccountStatus
import com.termoso.core.DeviceCard
import com.termoso.core.Fido2Listener
import com.termoso.core.LoginForm
import com.termoso.core.LoginOutcome
import com.termoso.core.MfaCard
import com.termoso.core.MfaMethod
import com.termoso.core.RegisterForm
import com.termoso.core.Registered
import com.termoso.core.SecurityKeyCredential
import com.termoso.core.SecurityKeyRequest
import com.termoso.core.SyncChange
import com.termoso.core.SyncListener
import com.termoso.core.SyncState
import com.termoso.core.SyncStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Termoso Cloud — the free hosted server. Same constant as the desktop client. */
const val CLOUD_URL = "https://app.termoso.com"

/** Which server a sign-in targets. */
enum class ServerChoice { Cloud, SelfHosted }

/**
 * Account and sync state of one opened vault. Rust owns the session, keys and
 * the sync engine; this class mirrors its status into flows, forwards change
 * notifications to [VaultRepository.revision] so lists reload, and restores
 * the persisted session on creation. Lives exactly as long as the open vault.
 */
class AccountManager(
    private val repo: VaultRepository,
    /** Runs before the Rust sign-out so account-bound sessions (shares, views) end first. */
    private val onSignOut: suspend () -> Unit = {},
) : SyncListener {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _status = MutableStateFlow(
        AccountStatus(account = null, pending = null, sync = idleSync(), vaults = emptyList()),
    )
    val status: StateFlow<AccountStatus> = _status.asStateFlow()

    /** True until the first [resume] attempt finished. */
    private val _restoring = MutableStateFlow(true)
    val restoring: StateFlow<Boolean> = _restoring.asStateFlow()

    private val _notices = MutableSharedFlow<String>(extraBufferCapacity = 4, onBufferOverflow = BufferOverflow.DROP_OLDEST)

    /** One-shot messages worth a snackbar (e.g. signed out by the server). */
    val notices: SharedFlow<String> = _notices.asSharedFlow()

    init {
        repo.app.setDeviceName(deviceName())
        repo.app.setSyncListener(this)
        scope.launch {
            try {
                refresh()
                runCatching { repo.app.accountResume() }
                    .onFailure { _notices.tryEmit("Could not restore the account session: ${it.userMessage()}") }
                refresh()
            } finally {
                _restoring.value = false
            }
        }
    }

    val signedIn: Boolean get() = _status.value.account != null

    suspend fun refresh(): AccountStatus =
        repo.read { accountStatus() }.also { _status.value = it }

    suspend fun login(server: String, email: String, password: String): LoginOutcome =
        repo.read { accountLogin(LoginForm(serverUrl = server, email = email, password = password)) }.also { done(it) }

    suspend fun register(server: String, email: String, password: String, displayName: String?, invite: String?): Registered =
        repo.read {
            accountRegister(
                RegisterForm(serverUrl = server, email = email, password = password, displayName = displayName, inviteToken = invite),
            )
        }.also { afterSignIn() }

    suspend fun mfa(method: MfaMethod, code: String): LoginOutcome =
        repo.read { accountMfa(method, code) }.also { done(it) }

    /**
     * Second factor with a FIDO2 security key over USB/NFC. Rust runs the
     * whole WebAuthn ceremony against the token; blocks until it is touched.
     * [deviceId] null = whichever attached key recognises the account.
     */
    suspend fun mfaSecurityKey(deviceId: String?, pin: String?, listener: Fido2Listener): LoginOutcome =
        repo.read { accountMfaSecurityKey(SecurityKeyRequest(deviceId = deviceId, pin = pin), listener) }.also { done(it) }

    suspend fun sendMfaEmail() = repo.read { accountMfaEmailSend() }

    suspend fun mfaStatus(): MfaCard = repo.read { accountMfaStatus() }

    suspend fun registerSecurityKey(name: String, deviceId: String, pin: String?, listener: Fido2Listener): SecurityKeyCredential =
        repo.read { accountRegisterSecurityKey(name, SecurityKeyRequest(deviceId = deviceId, pin = pin), listener) }

    suspend fun removeSecurityKey(id: String) = repo.read { accountRemoveSecurityKey(id) }

    suspend fun approveDevice(code: String): LoginOutcome =
        repo.read { accountApproveDevice(code) }.also { done(it) }

    suspend fun resendDeviceCode() = repo.read { accountResendDeviceCode() }

    suspend fun cancelLogin() {
        repo.read { accountCancelLogin() }
        refresh()
    }

    suspend fun signOut() {
        onSignOut()
        repo.read { accountSignOut() }
        refresh()
        repo.bump()
    }

    suspend fun syncNow(): SyncStatus = repo.read { syncNow() }.also { s -> _status.update { it.copy(sync = s) } }

    suspend fun devices(): List<DeviceCard> = repo.read { accountDevices() }

    suspend fun revokeDevice(id: String) = repo.read { accountRevokeDevice(id) }

    private suspend fun done(outcome: LoginOutcome) {
        if (outcome is LoginOutcome.Done) afterSignIn() else refresh()
    }

    private suspend fun afterSignIn() {
        refresh()
        repo.bump()
    }

    /** Stop listening; the Rust side stops its engine when the handle is closed. */
    fun close() {
        runCatching { repo.app.setSyncListener(null) }
        scope.cancel()
    }

    // ---- SyncListener (Rust worker threads) ----

    override fun onStatus(status: SyncStatus) {
        _status.update { it.copy(sync = status) }
    }

    override fun onChanged(change: SyncChange) {
        repo.bump()
        if (change is SyncChange.Vaults || change is SyncChange.Account) {
            scope.launch { runCatching { refresh() } }
        }
    }

    override fun onSignedOut(reason: String?) {
        scope.launch { runCatching { refresh() } }
        repo.bump()
        reason?.let { _notices.tryEmit(it) }
    }

    private fun deviceName(): String {
        val model = Build.MODEL.orEmpty().trim()
        val brand = Build.MANUFACTURER.orEmpty().trim()
        val name = when {
            model.isEmpty() -> "Android"
            model.startsWith(brand, ignoreCase = true) -> model
            brand.isEmpty() -> model
            else -> "${brand.replaceFirstChar { it.uppercase() }} $model"
        }
        return "$name (Android)"
    }

    private companion object {
        fun idleSync() = SyncStatus(
            state = SyncState.IDLE,
            realtime = false,
            lastSyncAt = null,
            lastError = null,
            pushed = 0u,
            pulled = 0u,
            conflicts = 0u,
        )
    }
}
