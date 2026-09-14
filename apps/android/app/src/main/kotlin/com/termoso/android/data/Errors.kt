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

/** `server 401: invalid_credentials: Wrong email or password` → the human part. */
private fun apiMessage(detail: String): String {
    val parts = detail.split(": ", limit = 3)
    return when {
        parts.size == 3 && parts[0].startsWith("server ") -> parts[2].ifBlank { parts[1].replace('_', ' ') }
        else -> detail
    }
}
