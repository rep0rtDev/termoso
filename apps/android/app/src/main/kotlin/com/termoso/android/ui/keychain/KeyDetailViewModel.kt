package com.termoso.android.ui.keychain

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.core.KeyItem
import com.termoso.core.SecurityKeyCard
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class KeyDetailState(
    val loading: Boolean = true,
    val key: KeyItem? = null,
    val publicKey: String = "",
    val securityKey: SecurityKeyCard? = null,
    val working: Boolean = false,
    val deleted: Boolean = false,
    val notice: String? = null,
)

/** One stored key: metadata, public half, and every mutation the Rust keychain offers. */
class KeyDetailViewModel(private val repo: VaultRepository, private val id: String) : ViewModel() {
    private val _state = MutableStateFlow(KeyDetailState())
    val state: StateFlow<KeyDetailState> = _state.asStateFlow()
    private var deleting = false

    init {
        viewModelScope.launch { repo.revision.collect { load() } }
    }

    private suspend fun load() {
        if (deleting) return
        runCatching {
            repo.read {
                val key = keys(null).firstOrNull { it.id == id } ?: error("Key not found")
                Triple(
                    key,
                    runCatching { publicKey(id) }.getOrDefault(key.publicKey),
                    if (key.keyType.startsWith("sk-")) runCatching { securityKeyInfo(id) }.getOrNull() else null,
                )
            }
        }.onSuccess { (k, pub, sk) -> _state.update { it.copy(loading = false, key = k, publicKey = pub, securityKey = sk) } }
            .onFailure { e -> _state.update { it.copy(loading = false, notice = e.userMessage()) } }
    }

    fun noticeShown() = _state.update { it.copy(notice = null) }

    private fun mutate(success: String?, block: suspend () -> Unit) {
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { block() }
                .onSuccess { _state.update { it.copy(working = false, notice = success) } }
                .onFailure { e -> _state.update { it.copy(working = false, notice = e.userMessage()) } }
        }
    }

    fun rename(label: String) = mutate(null) { repo.write { renameKey(id, label.trim()) } }

    fun changePassphrase(current: String?, next: String?, remember: Boolean) =
        mutate(if (next == null) "Passphrase removed" else "Passphrase changed") {
            repo.write { changeKeyPassphrase(id, current, next, remember) }
        }

    fun setCertificate(text: String?) =
        mutate(if (text == null) "Certificate removed" else "Certificate attached") {
            repo.write { setKeyCertificate(id, text?.trim()?.takeIf { it.isNotEmpty() }) }
        }

    fun delete() {
        deleting = true
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { repo.write { deleteKey(id) } }
                .onSuccess { _state.update { it.copy(working = false, deleted = true) } }
                .onFailure { e ->
                    deleting = false
                    _state.update { it.copy(working = false, notice = e.userMessage()) }
                }
        }
    }

    /** Returns the OpenSSH private key text or null after posting the error; never cached. */
    suspend fun exportPrivate(passphrase: String?, exportPassphrase: String?): String? =
        runCatching { repo.read { exportPrivateKey(id, passphrase, exportPassphrase) } }
            .onFailure { e -> _state.update { it.copy(notice = e.userMessage()) } }
            .getOrNull()
}
