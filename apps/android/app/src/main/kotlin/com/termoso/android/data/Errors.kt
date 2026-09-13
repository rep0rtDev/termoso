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
    is MobileException.Other -> detail
    else -> message ?: toString()
}
