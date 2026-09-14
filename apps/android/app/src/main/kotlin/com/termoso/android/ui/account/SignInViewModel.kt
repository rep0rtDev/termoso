package com.termoso.android.ui.account

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.AccountManager
import com.termoso.android.data.CLOUD_URL
import com.termoso.android.data.ServerChoice
import com.termoso.android.data.userMessage
import com.termoso.core.LoginOutcome
import com.termoso.core.MfaMethod
import com.termoso.core.ServerCard
import com.termoso.core.serverInfo
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

enum class AuthMode { SignIn, Register }

/** What the sign-in screen is showing. */
sealed interface AuthStep {
    data object Form : AuthStep
    data class Mfa(val methods: List<MfaMethod>) : AuthStep
    data class DeviceApproval(val emailHint: String) : AuthStep
    data class Recovery(val phrase: String) : AuthStep
}

/** Outcome of probing the server behind the URL field. */
sealed interface ServerProbe {
    data object Idle : ServerProbe
    data object Checking : ServerProbe
    data class Ok(val card: ServerCard) : ServerProbe
    data class Failed(val message: String) : ServerProbe
}

/**
 * Sign-in / sign-up form state. All credential handling happens in Rust:
 * the password only ever goes into `accountLogin` / `accountRegister`.
 */
class SignInViewModel(private val account: AccountManager, initialMode: AuthMode) : ViewModel() {
    var mode by mutableStateOf(initialMode)
    var server by mutableStateOf(ServerChoice.Cloud)
    var serverUrl by mutableStateOf("https://")
    var probe by mutableStateOf<ServerProbe>(ServerProbe.Idle)
    var email by mutableStateOf("")
    var password by mutableStateOf("")
    var confirm by mutableStateOf("")
    var displayName by mutableStateOf("")
    var invite by mutableStateOf("")
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var step by mutableStateOf<AuthStep>(AuthStep.Form)

    var mfaMethod by mutableStateOf<MfaMethod?>(null)
    var code by mutableStateOf("")
    var emailCodeSent by mutableStateOf(false)
    var recoverySaved by mutableStateOf(false)

    private var probeJob: Job? = null

    init {
        // A login interrupted mid-MFA / mid-approval is still pending in Rust.
        when (val pending = account.status.value.pending) {
            is LoginOutcome.MfaRequired -> enterMfa(pending.methods)
            is LoginOutcome.DeviceApprovalRequired -> step = AuthStep.DeviceApproval(pending.emailHint)
            else -> {}
        }
    }

    val effectiveUrl: String
        get() = if (server == ServerChoice.Cloud) CLOUD_URL else serverUrl.trim().trimEnd('/')

    fun chooseServer(choice: ServerChoice) {
        if (server == choice) return
        server = choice
        probe = ServerProbe.Idle
        error = null
        if (choice == ServerChoice.SelfHosted) scheduleProbe()
    }

    fun editServerUrl(url: String) {
        serverUrl = url
        probe = ServerProbe.Idle
        scheduleProbe()
    }

    /** Probe the self-hosted URL a moment after the user stops typing. */
    private fun scheduleProbe() {
        probeJob?.cancel()
        val url = serverUrl.trim()
        val hostPart = url.substringAfter("://", "")
        if (server != ServerChoice.SelfHosted || hostPart.isBlank() || !hostPart.any { it.isLetterOrDigit() }) return
        probeJob = viewModelScope.launch {
            delay(600)
            probeNow()
        }
    }

    suspend fun probeNow(): ServerCard? {
        val url = effectiveUrl
        probe = ServerProbe.Checking
        return runCatching { withContext(Dispatchers.IO) { serverInfo(url) } }
            .onSuccess { probe = ServerProbe.Ok(it) }
            .onFailure { probe = ServerProbe.Failed(it.userMessage()) }
            .getOrNull()
    }

    val serverCard: ServerCard? get() = (probe as? ServerProbe.Ok)?.card

    fun submit(onDone: () -> Unit) {
        if (busy) return
        val url = effectiveUrl
        val mail = email.trim()
        error = when {
            server == ServerChoice.SelfHosted && url.substringAfter("://", "").isBlank() -> "Enter your server address."
            mail.isEmpty() || !mail.contains('@') -> "Enter a valid email address."
            password.isEmpty() -> "Enter your password."
            mode == AuthMode.Register && password.length < 12 -> "Use at least 12 characters for the master password."
            mode == AuthMode.Register && password != confirm -> "Passwords do not match."
            else -> null
        }
        if (error != null) return
        run {
            when (mode) {
                AuthMode.SignIn -> handle(account.login(url, mail, password), onDone)
                AuthMode.Register -> {
                    val reg = account.register(
                        url,
                        mail,
                        password,
                        displayName.trim().ifEmpty { null },
                        invite.trim().ifEmpty { null },
                    )
                    password = ""
                    confirm = ""
                    step = AuthStep.Recovery(reg.recoveryPhrase)
                }
            }
        }
    }

    fun submitMfa(onDone: () -> Unit) {
        val method = mfaMethod ?: return
        val value = code.trim()
        if (value.isEmpty()) {
            error = "Enter the code."
            return
        }
        run { handle(account.mfa(method, value), onDone) }
    }

    fun sendEmailCode() {
        run {
            account.sendMfaEmail()
            emailCodeSent = true
        }
    }

    fun submitApproval(onDone: () -> Unit) {
        val value = code.trim()
        if (value.isEmpty()) {
            error = "Enter the code from the email."
            return
        }
        run { handle(account.approveDevice(value), onDone) }
    }

    fun resendApproval() {
        run { account.resendDeviceCode() }
    }

    fun pickMethod(method: MfaMethod) {
        mfaMethod = method
        code = ""
        error = null
    }

    /** Back out of MFA / device approval; the pending Rust flow is dropped. */
    fun cancelPending() {
        viewModelScope.launch {
            runCatching { account.cancelLogin() }
            step = AuthStep.Form
            code = ""
            password = ""
            error = null
        }
    }

    private fun handle(outcome: LoginOutcome, onDone: () -> Unit) {
        when (outcome) {
            is LoginOutcome.Done -> {
                password = ""
                code = ""
                onDone()
            }
            is LoginOutcome.MfaRequired -> {
                password = ""
                enterMfa(outcome.methods)
            }
            is LoginOutcome.DeviceApprovalRequired -> {
                password = ""
                code = ""
                step = AuthStep.DeviceApproval(outcome.emailHint)
            }
        }
    }

    private fun enterMfa(methods: List<MfaMethod>) {
        step = AuthStep.Mfa(methods)
        mfaMethod = methods.firstOrNull { it != MfaMethod.WEBAUTHN } ?: methods.firstOrNull()
        code = ""
        emailCodeSent = false
    }

    fun switchMode(next: AuthMode) {
        mode = next
        error = null
    }

    private fun run(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        error = null
        viewModelScope.launch {
            try {
                block()
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                error = e.userMessage()
            } finally {
                busy = false
            }
        }
    }
}
