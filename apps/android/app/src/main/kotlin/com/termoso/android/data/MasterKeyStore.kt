package com.termoso.android.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import com.termoso.core.generateMasterKey
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The 32-byte vault master key, wrapped by a non-exportable AES-GCM key that
 * lives in the Android Keystore (StrongBox when available). Only the wrapped
 * blob touches disk; the plaintext key exists in memory just long enough to
 * hand it to Rust, which zeroizes it after opening the store.
 */
class MasterKeyStore(context: Context) {
    private val file = File(context.noBackupFilesDir, "master.key")

    fun exists(): Boolean = file.exists()

    /** Generate a fresh master key in Rust, wrap it and persist the blob. */
    fun create(): ByteArray {
        val key = generateMasterKey()
        wrap(key)
        return key
    }

    /** Unwrap the stored master key. Throws if the Keystore key is gone. */
    fun unwrap(): ByteArray {
        val blob = file.readBytes()
        require(blob.size > IV_LEN) { "master key blob is truncated" }
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.DECRYPT_MODE, keystoreKey(), GCMParameterSpec(TAG_BITS, blob, 0, IV_LEN))
        val key = cipher.doFinal(blob, IV_LEN, blob.size - IV_LEN)
        check(key.size == 32) { "master key has wrong length" }
        return key
    }

    /** Remove both the wrapped blob and the Keystore key (profile reset). */
    fun destroy() {
        file.delete()
        KeyStore.getInstance(PROVIDER).apply { load(null) }.let {
            if (it.containsAlias(ALIAS)) it.deleteEntry(ALIAS)
        }
    }

    private fun wrap(key: ByteArray) {
        val cipher = Cipher.getInstance(TRANSFORM)
        cipher.init(Cipher.ENCRYPT_MODE, keystoreKey())
        val iv = cipher.iv
        check(iv.size == IV_LEN)
        val ciphertext = cipher.doFinal(key)
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeBytes(iv + ciphertext)
        if (!tmp.renameTo(file)) {
            file.writeBytes(iv + ciphertext)
            tmp.delete()
        }
    }

    private fun keystoreKey(): SecretKey {
        val ks = KeyStore.getInstance(PROVIDER).apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setRandomizedEncryptionRequired(true)
            .build()
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(spec)
        return generator.generateKey()
    }

    private companion object {
        const val PROVIDER = "AndroidKeyStore"
        const val ALIAS = "termoso.master"
        const val TRANSFORM = "AES/GCM/NoPadding"
        const val IV_LEN = 12
        const val TAG_BITS = 128
    }
}
