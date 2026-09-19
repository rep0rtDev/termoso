package com.termoso.android.ui.security

import android.content.Context
import android.content.ContextWrapper
import androidx.biometric.BiometricManager
import androidx.biometric.BiometricManager.Authenticators.BIOMETRIC_STRONG
import androidx.biometric.BiometricManager.Authenticators.DEVICE_CREDENTIAL
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import com.termoso.android.R
import com.termoso.android.str
import kotlin.coroutines.resume
import kotlinx.coroutines.suspendCancellableCoroutine

/** Outcome of a device-authentication prompt (biometric or screen-lock credential). */
sealed interface AuthResult {
    data object Success : AuthResult
    data object Cancelled : AuthResult
    data class Failed(val message: String) : AuthResult
}

private const val AUTHENTICATORS = BIOMETRIC_STRONG or DEVICE_CREDENTIAL

/** Null when the device can prompt; otherwise a sentence telling the user what to set up. */
fun deviceAuthProblem(context: Context): String? =
    when (BiometricManager.from(context).canAuthenticate(AUTHENTICATORS)) {
        BiometricManager.BIOMETRIC_SUCCESS -> null
        BiometricManager.BIOMETRIC_ERROR_NONE_ENROLLED ->
            str(R.string.set_up_a_screen_lock_pin_pattern_or)
        BiometricManager.BIOMETRIC_ERROR_NO_HARDWARE,
        BiometricManager.BIOMETRIC_ERROR_HW_UNAVAILABLE,
        -> str(R.string.this_device_cannot_authenticate_you)
        BiometricManager.BIOMETRIC_ERROR_SECURITY_UPDATE_REQUIRED -> str(R.string.a_security_update_is_required_for_device_authentication)
        else -> str(R.string.device_authentication_is_unavailable)
    }

/**
 * Show the system prompt and suspend until it closes. Uses the biometric or
 * device credential the user has enrolled; no secret is derived from it — the
 * Keystore key that wraps the vault master key checks the same authentication.
 */
suspend fun authenticateDevice(activity: FragmentActivity, title: String, subtitle: String? = null): AuthResult =
    suspendCancellableCoroutine { cont ->
        val prompt = BiometricPrompt(
            activity,
            ContextCompat.getMainExecutor(activity),
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    if (cont.isActive) cont.resume(AuthResult.Success)
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    if (!cont.isActive) return
                    val cancelled = errorCode == BiometricPrompt.ERROR_USER_CANCELED ||
                        errorCode == BiometricPrompt.ERROR_NEGATIVE_BUTTON ||
                        errorCode == BiometricPrompt.ERROR_CANCELED
                    cont.resume(if (cancelled) AuthResult.Cancelled else AuthResult.Failed(errString.toString()))
                }
            },
        )
        val info = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .apply { if (subtitle != null) setSubtitle(subtitle) }
            .setAllowedAuthenticators(AUTHENTICATORS)
            .setConfirmationRequired(false)
            .build()
        prompt.authenticate(info)
        cont.invokeOnCancellation { prompt.cancelAuthentication() }
    }

/** The hosting [FragmentActivity] of a Compose context (BiometricPrompt needs one). */
fun Context.findFragmentActivity(): FragmentActivity? {
    var ctx: Context = this
    while (ctx is ContextWrapper) {
        if (ctx is FragmentActivity) return ctx
        ctx = ctx.baseContext
    }
    return null
}
