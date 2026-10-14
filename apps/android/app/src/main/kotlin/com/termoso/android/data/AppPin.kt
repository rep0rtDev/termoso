package com.termoso.android.data

import android.content.Context
import android.os.SystemClock
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.PBEKeySpec

/**
 * Optional in-app PIN or pattern that gates the UI in addition to (or instead of)
 * the device credential. Only a salted PBKDF2 hash lives on disk; the vault keys
 * are not derived from it, so it is a privacy screen, not the encryption secret.
 */
class AppPin(context: Context) {
    enum class Kind { PIN, PATTERN }

    private val prefs = context.applicationContext.getSharedPreferences("app_pin", Context.MODE_PRIVATE)
    private var failures = 0
    private var blockedUntil = 0L

    val isSet: Boolean get() = prefs.contains(HASH)
    val kind: Kind? get() = prefs.getString(KIND, null)?.let { runCatching { Kind.valueOf(it) }.getOrNull() }

    fun set(code: String, kind: Kind) {
        val salt = ByteArray(16).also { SecureRandom().nextBytes(it) }
        prefs.edit()
            .putString(SALT, salt.hex())
            .putString(HASH, derive(code, salt).hex())
            .putString(KIND, kind.name)
            .apply()
        failures = 0
    }

    fun clear() {
        prefs.edit().clear().apply()
        failures = 0
    }

    /** Seconds left in the lock-out after repeated wrong codes, 0 when a try is allowed. */
    fun retryIn(): Long = ((blockedUntil - SystemClock.elapsedRealtime()) / 1000).coerceAtLeast(0)

    fun verify(code: String): Boolean {
        if (retryIn() > 0) return false
        val salt = prefs.getString(SALT, null)?.unhex() ?: return false
        val stored = prefs.getString(HASH, null)?.unhex() ?: return false
        val ok = MessageDigest.isEqual(derive(code, salt), stored)
        if (ok) {
            failures = 0
        } else {
            failures++
            if (failures >= 5) blockedUntil = SystemClock.elapsedRealtime() + 30_000L * (1 shl (failures - 5).coerceAtMost(4))
        }
        return ok
    }

    private fun derive(code: String, salt: ByteArray): ByteArray =
        SecretKeyFactory.getInstance("PBKDF2WithHmacSHA256")
            .generateSecret(PBEKeySpec(code.toCharArray(), salt, 120_000, 256))
            .encoded

    private fun ByteArray.hex() = joinToString("") { "%02x".format(it) }
    private fun String.unhex() = chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private companion object {
        const val SALT = "salt"
        const val HASH = "hash"
        const val KIND = "kind"
    }
}
