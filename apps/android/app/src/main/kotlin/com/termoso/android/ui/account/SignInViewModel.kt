package com.termoso.android.ui.account

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.data.CLOUD_URL
import com.termoso.android.data.ServerChoice
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.Fido2Listener
import com.termoso.core.LoginOutcome
import com.termoso.core.MfaMethod
import com.termoso.core.MobileException
import com.termoso.core.ServerCard
import com.termoso.core.SsoOutcome
import com.termoso.core.SsoProviderCard
import com.termoso.core.serverInfo
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

/** How often the server is asked whether the browser finished, and for how long. */
private const val SSO_POLL_MS = 2_000L
private const val SSO_TIMEOUT_MS = 10 * 60_000L

enum class AuthMode { SignIn, Register }

/** What the sign-in screen is showing. */
sealed interface AuthStep {
    data object Form : AuthStep
    data class Mfa(val methods: List<MfaMethod>) : AuthStep
    data class DeviceApproval(val emailHint: String) : AuthStep
    data class Recovery(val phrase: String) : AuthStep
}

/**
 * Where a single sign-on stands. Only public facts live here: the provider,
 * the flow id (also visible in the browser's history) and the verified email /
 * name the server reported. The SSO session itself stays in Rust.
 */
sealed interface SsoState {
    data object Idle : SsoState

    /** The browser is open; [url] is what the sign-in screen launches. */
    data class Waiting(val provider: SsoProviderCard, val flowId: String, val url: String) : SsoState

    /** Identity verified; the Termoso password finishes the login / sign-up. */
    data class Verified(val provider: SsoProviderCard, val email: String, val displayName: String?, val newAccount: Boolean) : SsoState
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
class SignInViewModel(
    private val account: AccountManager,
    initialMode: AuthMode,
    /** `termoso://sso?flow=<id>` callbacks routed by the activity; consumed via [consumeSso]. */
    private val ssoCallbacks: StateFlow<String?>,
    private val consumeSso: (String) -> Unit,
) : ViewModel() {
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

    /** Security-key MFA: chosen token (null = any attached), its PIN, and the "touch now" moment. */
    var skDeviceId by mutableStateOf<String?>(null)
    var skPin by mutableStateOf("")
    var skTouch by mutableStateOf(false)

    var sso by mutableStateOf<SsoState>(SsoState.Idle)

    /** Set when the sign-in screen should open [SsoState.Waiting.url]; cleared by [browserOpened]. */
    var openBrowser by mutableStateOf<String?>(null)

    private var probeJob: Job? = null
    private var ssoJob: Job? = null

    init {
        // A login interrupted mid-MFA / mid-approval is still pending in Rust.
        when (val pending = account.status.value.pending) {
            is LoginOutcome.MfaRequired -> enterMfa(pending.methods)
            is LoginOutcome.DeviceApprovalRequired -> step = AuthStep.DeviceApproval(pending.emailHint)
            else -> {}
        }
        if (server == ServerChoice.Cloud) scheduleProbe()
        viewModelScope.launch {
            ssoCallbacks.filterNotNull().collect { flowId ->
                consumeSso(flowId)
                onSsoCallback(flowId)
            }
        }
    }

    val effectiveUrl: String
        get() = if (server == ServerChoice.Cloud) CLOUD_URL else serverUrl.trim().trimEnd('/')

    fun chooseServer(choice: ServerChoice) {
        if (server == choice) return
        server = choice
        probe = ServerProbe.Idle
        error = null
        cancelSso()
        scheduleProbe()
    }

    fun editServerUrl(url: String) {
        serverUrl = url
        probe = ServerProbe.Idle
        cancelSso()
        scheduleProbe()
    }

    /**
     * Probe the server a moment after the user stops typing (or at once for the
     * cloud): the result tells whether registration is open and which SSO
     * providers to offer.
     */
    private fun scheduleProbe() {
        probeJob?.cancel()
        if (server == ServerChoice.SelfHosted) {
            val hostPart = serverUrl.trim().substringAfter("://", "")
            if (hostPart.isBlank() || !hostPart.any { it.isLetterOrDigit() }) return
        }
        probeJob = viewModelScope.launch {
            if (server == ServerChoice.SelfHosted) delay(600)
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

    /** Providers the probed server advertises; empty while unknown. */
    val ssoProviders: List<SsoProviderCard> get() = serverCard?.ssoProviders.orEmpty()

    /** Sign-up needs an invitation: registration is closed and this is not an SSO identity the server lets in. */
    val needsInvite: Boolean
        get() {
            val card = serverCard ?: return false
            val verified = sso as? SsoState.Verified
            return !card.registrationOpen && !(verified?.newAccount == true && card.ssoRegistration)
        }

    fun submit(onDone: () -> Unit) {
        if (busy) return
        val url = effectiveUrl
        val mail = email.trim()
        val viaSso = sso is SsoState.Verified
        error = when {
            server == ServerChoice.SelfHosted && url.substringAfter("://", "").isBlank() -> str(R.string.enter_your_server_address)
            mail.isEmpty() || !mail.contains('@') -> str(R.string.enter_a_valid_email_address)
            password.isEmpty() -> str(R.string.enter_your_password)
            mode == AuthMode.Register && password.length < 12 -> str(R.string.use_at_least_12_characters_for_the_master)
            mode == AuthMode.Register && password != confirm -> str(R.string.passwords_do_not_match)
            else -> null
        }
        if (error != null) return
        run {
            try {
                when (mode) {
                    AuthMode.SignIn -> handle(account.login(url, mail, password, sso = viaSso), onDone)
                    AuthMode.Register -> {
                        val reg = account.register(
                            url,
                            mail,
                            password,
                            displayName.trim().ifEmpty { null },
                            invite.trim().ifEmpty { null },
                            sso = viaSso,
                        )
                        password = ""
                        confirm = ""
                        step = AuthStep.Recovery(reg.recoveryPhrase)
                    }
                }
            } finally {
                // Rust hands the SSO session over exactly once, whatever the outcome.
                if (viaSso) sso = SsoState.Idle
            }
        }
    }

    /**
     * "Continue with <provider>": Rust starts the flow against the chosen server
     * and the screen opens the returned URL in a Custom Tab. Polling runs in
     * the background so a user who lands back in the app without the deep link
     * (browser closed, tab switched) still gets the result.
     */
    fun startSso(provider: SsoProviderCard) {
        if (busy) return
        val url = effectiveUrl
        if (server == ServerChoice.SelfHosted && url.substringAfter("://", "").isBlank()) {
            error = str(R.string.enter_your_server_address)
            return
        }
        cancelSso()
        run {
            val started = account.ssoStart(url, provider.id)
            sso = SsoState.Waiting(provider, started.flowId, started.authorizationUrl)
            openBrowser = started.authorizationUrl
            ssoJob = viewModelScope.launch { pollSso(provider, started.flowId) }
        }
    }

    fun browserOpened() {
        openBrowser = null
    }

    fun reopenBrowser() {
        openBrowser = (sso as? SsoState.Waiting)?.url
    }

    fun browserUnavailable() {
        val waiting = sso as? SsoState.Waiting ?: return
        settleSso(waiting.provider, waiting.flowId, str(R.string.no_browser_available))
    }

    private fun waitingFor(flowId: String): SsoState.Waiting? = (sso as? SsoState.Waiting)?.takeIf { it.flowId == flowId }

    private suspend fun pollSso(provider: SsoProviderCard, flowId: String) {
        val settled = withTimeoutOrNull(SSO_TIMEOUT_MS) {
            while (waitingFor(flowId) != null) {
                delay(SSO_POLL_MS)
                val outcome = try {
                    account.ssoPoll()
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    settleSso(provider, flowId, e.userMessage())
                    return@withTimeoutOrNull
                }
                applySso(provider, flowId, outcome)
            }
        }
        if (settled == null) settleSso(provider, flowId, str(R.string.sso_timed_out))
    }

    private suspend fun onSsoCallback(flowId: String) {
        val waiting = waitingFor(flowId)
        if (waiting == null) {
            // Not the flow this screen is waiting for (stale callback, app restarted): nothing to hand over.
            if (sso == SsoState.Idle) error = str(R.string.sso_expired)
            return
        }
        val outcome = try {
            account.ssoCallback(flowId)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            settleSso(waiting.provider, flowId, e.userMessage())
            return
        }
        applySso(waiting.provider, flowId, outcome)
    }

    /** Move the screen on once the server reports a terminal [outcome]. */
    private fun applySso(provider: SsoProviderCard, flowId: String, outcome: SsoOutcome) {
        if (waitingFor(flowId) == null) return
        when (outcome) {
            SsoOutcome.Pending -> return
            is SsoOutcome.LoginRequired -> {
                sso = SsoState.Verified(provider, outcome.email, null, newAccount = false)
                mode = AuthMode.SignIn
                email = outcome.email
                error = null
            }
            is SsoOutcome.RegistrationRequired -> {
                sso = SsoState.Verified(provider, outcome.email, outcome.displayName, newAccount = true)
                mode = AuthMode.Register
                email = outcome.email
                if (displayName.isBlank()) displayName = outcome.displayName.orEmpty()
                error = null
            }
            is SsoOutcome.Failed -> settleSso(provider, flowId, outcome.message)
        }
        ssoJob?.cancel()
        ssoJob = null
    }

    private fun settleSso(provider: SsoProviderCard, flowId: String, message: String) {
        if (waitingFor(flowId)?.provider != provider) return
        sso = SsoState.Idle
        error = message
        viewModelScope.launch { runCatching { account.ssoCancel() } }
    }

    /** Drop the current SSO attempt (browser abandoned, or the user wants the password form back). */
    fun cancelSso() {
        ssoJob?.cancel()
        ssoJob = null
        openBrowser = null
        if (sso == SsoState.Idle) return
        sso = SsoState.Idle
        error = null
        viewModelScope.launch { runCatching { account.ssoCancel() } }
    }

    fun submitMfa(onDone: () -> Unit) {
        val method = mfaMethod ?: return
        val value = code.trim()
        if (value.isEmpty()) {
            error = str(R.string.enter_the_code)
            return
        }
        run { handle(account.mfa(method, value), onDone) }
    }

    /**
     * Second factor with the attached security key. No code to type: Rust
     * fetches the WebAuthn challenge, has the token sign it and finishes the
     * login. The PIN goes straight to Rust and is cleared afterwards.
     */
    fun submitSecurityKey(onDone: () -> Unit) {
        val pin = skPin.takeIf { it.isNotEmpty() }
        skTouch = false
        val listener = object : Fido2Listener {
            override fun onTouch() {
                skTouch = true
            }
        }
        run {
            try {
                val outcome = account.mfaSecurityKey(skDeviceId, pin, listener)
                skPin = ""
                handle(outcome, onDone)
            } catch (e: MobileException.SecurityKey) {
                if (e.kind.startsWith("fido2_pin")) skPin = ""
                throw e
            } finally {
                skTouch = false
            }
        }
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
            error = str(R.string.enter_the_code_from_the_email)
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
        skPin = ""
        error = null
    }

    /** Back out of MFA / device approval; the pending Rust flow is dropped. */
    fun cancelPending() {
        cancelSso()
        viewModelScope.launch {
            runCatching { account.cancelLogin() }
            step = AuthStep.Form
            code = ""
            skPin = ""
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
        if (mode == next) return
        mode = next
        error = null
        cancelSso()
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
