package com.termoso.android.data

import com.termoso.core.MobileException

/** Human-readable text for any failure coming out of the core (or elsewhere). */
fun Throwable.userMessage(): String = when (this) {
    is MobileException.Invalid -> detail
    is MobileException.NotFound -> detail
    is MobileException.Locked -> "The vault is locked."
    is MobileException.Ssh -> detail
    is MobileException.AuthFailed -> "Authentication failed ($remaining)."
    is MobileException.HostKeyRejected -> detail
    is MobileException.Key -> detail
    is MobileException.Cancelled -> "Cancelled."
    is MobileException.Closed -> "Connection closed."
    is MobileException.ReauthRequired -> "Confirm your password to make this change."
    is MobileException.SecurityKey -> securityKeyMessage(kind, detail, retries)
    is MobileException.Other -> when (kind) {
        "api" -> apiMessage(detail)
        "network" -> "Could not reach the server. Check the address and your connection."
        "websocket" -> "Realtime connection failed."
        "vault_read_only" -> "This vault is view-only for you."
        "not_signed_in" -> "Not signed in."
        else -> detail
    }
    else -> message ?: toString()
}

/** Typed FIDO2 failures (`Fido2Error::kind`) in the words the user needs to act on them. */
private fun securityKeyMessage(kind: String, detail: String, retries: Int?): String = when (kind) {
    "fido2_no_device" -> "No security key found. Plug one in over USB or hold it to the back of the phone."
    "fido2_device_gone" -> "The security key was disconnected."
    "fido2_pin_required" -> "This security key needs its PIN."
    "fido2_pin_invalid" -> "Wrong PIN." + (retries?.let { " $it attempt${if (it == 1) "" else "s"} left." } ?: "")
    "fido2_pin_blocked" -> "The PIN is blocked. Remove and reinsert the key, or reset it."
    "fido2_pin_not_set" -> "This security key has no PIN yet; set one with another tool first."
    "fido2_timeout" -> "The security key was not touched in time."
    "fido2_denied" -> "The security key refused the request."
    "fido2_wrong_device" -> "This key does not belong to that security key."
    else -> detail
}

/** `server 401: invalid_credentials: Wrong email or password` → the human part. */
private fun apiMessage(detail: String): String {
    val parts = detail.split(": ", limit = 3)
    return when {
        parts.size == 3 && parts[0].startsWith("server ") -> parts[2].ifBlank { parts[1].replace('_', ' ') }
        else -> detail
    }
}
