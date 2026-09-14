package com.termoso.android.data

import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.UserNotAuthenticatedException
import com.termoso.core.generateMasterKey
import java.io.File
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.security.PublicKey
import java.security.spec.MGF1ParameterSpec
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.OAEPParameterSpec
import javax.crypto.spec.PSource

/**
 * The 32-byte vault master key, wrapped by a non-exportable key that lives in
 * the Android Keystore. Only the wrapped blob touches disk; the plaintext key
 * exists in memory just long enough to hand it to Rust, which zeroizes it after
 * opening the store.
 *
 * Two wrapping modes:
 *  - plain: AES-GCM key, usable any time the device is unlocked (`master.key`);
 *  - app lock: RSA-OAEP key pair whose private half demands a fresh device
 *    authentication (biometric or screen-lock credential) before every unwrap
 *    (`master.auth.key`). Wrapping uses the public half, so enabling app lock
 *    needs no prompt while disabling it does.
 *
 * When an unwrap needs the user first, [unwrap] throws [UserNotAuthenticatedException];
 * the UI shows a BiometricPrompt and retries within [AUTH_WINDOW_SECONDS].
 */
class MasterKeyStore(context: Context) {
    private val plainFile = File(context.noBackupFilesDir, "master.key")
    private val authFile = File(context.noBackupFilesDir, "master.auth.key")

    fun exists(): Boolean = plainFile.exists() || authFile.exists()

    /** True when opening the vault requires device authentication. */
    fun authRequired(): Boolean = authFile.exists()

    /** Generate a fresh master key in Rust, wrap it and persist the blob. */
    fun create(): ByteArray {
        val key = generateMasterKey()
        wrapPlain(key)
        return key
    }

    /**
     * Unwrap the stored master key. Throws [UserNotAuthenticatedException] when
     * app lock is on and the user has not authenticated recently.
     */
    fun unwrap(): ByteArray = if (authFile.exists()) unwrapAuth() else unwrapPlain()

    /**
     * Switch wrapping modes. Turning app lock off re-reads the key through the
     * auth-bound wrapper, so the caller must have authenticated the user first.
     */
    fun setAuthRequired(required: Boolean) {
        if (required == authRequired()) return
        val key = unwrap()
        try {
            if (required) {
                wrapAuth(key)
                plainFile.delete()
                deleteAlias(PLAIN_ALIAS)
            } else {
                wrapPlain(key)
                authFile.delete()
                deleteAlias(AUTH_ALIAS)
            }
        } finally {
            key.fill(0)
        }
    }

    /** Remove the wrapped blobs and the Keystore keys (profile reset). */
    fun destroy() {
        plainFile.delete()
        authFile.delete()
        deleteAlias(PLAIN_ALIAS)
        deleteAlias(AUTH_ALIAS)
    }

    private fun unwrapPlain(): ByteArray {
        val blob = plainFile.readBytes()
        require(blob.size > IV_LEN) { "master key blob is truncated" }
        val cipher = Cipher.getInstance(AES_TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, aesKey(), GCMParameterSpec(TAG_BITS, blob, 0, IV_LEN))
        return checkKey(cipher.doFinal(blob, IV_LEN, blob.size - IV_LEN))
    }

    private fun unwrapAuth(): ByteArray {
        val blob = authFile.readBytes()
        val cipher = Cipher.getInstance(RSA_TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, rsaPrivateKey(), OAEP)
        return checkKey(cipher.doFinal(blob))
    }

    private fun checkKey(key: ByteArray): ByteArray {
        check(key.size == 32) { "master key has wrong length" }
        return key
    }

    private fun wrapPlain(key: ByteArray) {
        val cipher = Cipher.getInstance(AES_TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, aesKey())
        val iv = cipher.iv
        check(iv.size == IV_LEN)
        atomicWrite(plainFile, iv + cipher.doFinal(key))
    }

    private fun wrapAuth(key: ByteArray) {
        val cipher = Cipher.getInstance(RSA_TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, rsaPublicKey(), OAEP)
        atomicWrite(authFile, cipher.doFinal(key))
    }

    private fun atomicWrite(file: File, bytes: ByteArray) {
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeBytes(bytes)
        if (!tmp.renameTo(file)) {
            file.writeBytes(bytes)
            tmp.delete()
        }
    }

    private fun keyStore(): KeyStore = KeyStore.getInstance(PROVIDER).apply { load(null) }

    private fun deleteAlias(alias: String) {
        keyStore().let { if (it.containsAlias(alias)) it.deleteEntry(alias) }
    }

    private fun aesKey(): SecretKey {
        (keyStore().getEntry(PLAIN_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val spec = KeyGenParameterSpec.Builder(PLAIN_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setRandomizedEncryptionRequired(true)
            .build()
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(spec)
        return generator.generateKey()
    }

    private fun rsaPrivateKey(): PrivateKey =
        (keyStore().getEntry(AUTH_ALIAS, null) as? KeyStore.PrivateKeyEntry)?.privateKey
            ?: error("app lock key is missing from the Keystore")

    private fun rsaPublicKey(): PublicKey {
        (keyStore().getEntry(AUTH_ALIAS, null) as? KeyStore.PrivateKeyEntry)?.let { return it.certificate.publicKey }
        val builder = KeyGenParameterSpec.Builder(AUTH_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setKeySize(2048)
            .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA1)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_OAEP)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(false)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setUserAuthenticationParameters(
                AUTH_WINDOW_SECONDS,
                KeyProperties.AUTH_BIOMETRIC_STRONG or KeyProperties.AUTH_DEVICE_CREDENTIAL,
            )
        } else {
            @Suppress("DEPRECATION")
            builder.setUserAuthenticationValidityDurationSeconds(AUTH_WINDOW_SECONDS)
        }
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_RSA, PROVIDER)
        generator.initialize(builder.build())
        return generator.generateKeyPair().public
    }

    companion object {
        /** Seconds a device authentication stays valid for the auth-bound key. */
        const val AUTH_WINDOW_SECONDS = 30

        private const val PROVIDER = "AndroidKeyStore"
        private const val PLAIN_ALIAS = "termoso.master"
        private const val AUTH_ALIAS = "termoso.master.auth"
        private const val AES_TRANSFORM = "AES/GCM/NoPadding"
        private const val RSA_TRANSFORM = "RSA/ECB/OAEPWithSHA-256AndMGF1Padding"
        private const val IV_LEN = 12
        private const val TAG_BITS = 128
        private val OAEP = OAEPParameterSpec("SHA-256", "MGF1", MGF1ParameterSpec.SHA1, PSource.PSpecified.DEFAULT)
    }
}
