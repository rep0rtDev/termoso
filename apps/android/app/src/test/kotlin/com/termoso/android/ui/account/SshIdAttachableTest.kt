package com.termoso.android.ui.account

import com.termoso.core.KeyItem
import org.junit.Assert.assertEquals
import org.junit.Test

class SshIdAttachableTest {
    private fun key(
        id: String,
        securityKey: Boolean = true,
        encrypted: Boolean = false,
        hasPassphrase: Boolean = false,
        unreadable: Boolean = false,
        publicKey: String = "sk-ssh-ed25519@openssh.com AAAA$id",
    ) = KeyItem(
        id = id,
        vaultId = "v",
        label = id,
        keyType = if (securityKey) "sk-ssh-ed25519@openssh.com" else "ssh-ed25519",
        bits = 256u,
        fingerprint = "SHA256:$id",
        publicKey = publicKey,
        comment = "",
        encrypted = encrypted,
        hasPassphrase = hasPassphrase,
        unreadable = unreadable,
        usedBy = 0u,
        hasCertificate = false,
        securityKey = securityKey,
        updatedAt = 0,
    )

    @Test
    fun onlyReadableSecurityKeysNotYetPublishedAreOffered() {
        val all = listOf(
            key("sk"),
            key("soft", securityKey = false),
            key("broken", unreadable = true),
            key("locked", encrypted = true),
            key("remembered", encrypted = true, hasPassphrase = true),
            key("published", publicKey = "sk-ssh-ed25519@openssh.com AAAApub "),
        )
        val published = setOf("sk-ssh-ed25519@openssh.com AAAApub")

        assertEquals(listOf("sk", "remembered"), attachableKeys(all, published).map { it.id })
    }
}
