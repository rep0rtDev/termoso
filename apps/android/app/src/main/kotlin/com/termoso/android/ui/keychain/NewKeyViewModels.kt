package com.termoso.android.ui.keychain

import androidx.annotation.StringRes
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.R
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.KeyAlgorithm
import com.termoso.core.KeyGenerateDraft
import com.termoso.core.KeyImportDraft
import com.termoso.core.KeyPreview
import com.termoso.core.VaultInfo
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Algorithm choices offered by the generator, in display order. */
enum class KeyKind(val label: String, @StringRes val hint: Int) {
    ED25519("Ed25519", R.string.key_kind_ed25519_hint),
    RSA("RSA", R.string.key_kind_rsa_hint),
    ECDSA("ECDSA", R.string.key_kind_ecdsa_hint),
}

data class GenerateKeyState(
    val vaults: List<VaultInfo> = emptyList(),
    val vaultId: String? = null,
    val label: String = "",
    val kind: KeyKind = KeyKind.ED25519,
    val rsaBits: UInt = 3072u,
    val ecdsaBits: UInt = 256u,
    val comment: String = "",
    val passphrase: String = "",
    val confirm: String = "",
    val remember: Boolean = true,
    val working: Boolean = false,
    val savedId: String? = null,
    val error: String? = null,
) {
    val passphraseMismatch get() = passphrase.isNotEmpty() && confirm.isNotEmpty() && passphrase != confirm
    val canSave get() = !working && vaultId != null && (passphrase.isEmpty() || passphrase == confirm)

    fun algorithm(): KeyAlgorithm = when (kind) {
        KeyKind.ED25519 -> KeyAlgorithm.Ed25519
        KeyKind.RSA -> KeyAlgorithm.Rsa(rsaBits)
        KeyKind.ECDSA -> if (ecdsaBits == 384u) KeyAlgorithm.EcdsaP384 else KeyAlgorithm.EcdsaP256
    }
}

class GenerateKeyViewModel(private val repo: VaultRepository, initialVault: String?) : ViewModel() {
    private val _state = MutableStateFlow(GenerateKeyState(vaultId = initialVault))
    val state: StateFlow<GenerateKeyState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            runCatching { repo.read { vaults().filter { !it.locked } } }
                .onSuccess { v -> _state.update { it.copy(vaults = v, vaultId = it.vaultId ?: v.firstOrNull()?.id) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun update(transform: (GenerateKeyState) -> GenerateKeyState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    fun generate() {
        val s = _state.value
        val vault = s.vaultId ?: return
        if (!s.canSave) return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching {
                repo.write {
                    generateKey(
                        KeyGenerateDraft(
                            vaultId = vault,
                            label = s.label.trim().ifBlank { defaultLabel(s) },
                            algorithm = s.algorithm(),
                            comment = s.comment.trim(),
                            passphrase = s.passphrase.takeIf { it.isNotEmpty() },
                            rememberPassphrase = s.passphrase.isNotEmpty() && s.remember,
                        ),
                    )
                }
            }.onSuccess { k -> _state.update { it.copy(working = false, savedId = k.id) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }

    private fun defaultLabel(s: GenerateKeyState) = when (s.kind) {
        KeyKind.ED25519 -> str(R.string.ed25519_key)
        KeyKind.RSA -> str(R.string.rsa_key, s.rsaBits)
        KeyKind.ECDSA -> str(R.string.ecdsa_p_key, s.ecdsaBits)
    }
}

data class ImportKeyState(
    val vaults: List<VaultInfo> = emptyList(),
    val vaultId: String? = null,
    val label: String = "",
    val privateKey: String = "",
    val preview: KeyPreview? = null,
    val previewError: String? = null,
    val passphrase: String = "",
    val remember: Boolean = true,
    val certificate: String = "",
    val working: Boolean = false,
    val savedId: String? = null,
    val error: String? = null,
) {
    val canSave get() = !working && vaultId != null && privateKey.isNotBlank() && preview != null
}

class ImportKeyViewModel(private val repo: VaultRepository, initialVault: String?) : ViewModel() {
    private val _state = MutableStateFlow(ImportKeyState(vaultId = initialVault))
    val state: StateFlow<ImportKeyState> = _state.asStateFlow()
    private var inspectJob: Job? = null

    init {
        viewModelScope.launch {
            runCatching { repo.read { vaults().filter { !it.locked } } }
                .onSuccess { v -> _state.update { it.copy(vaults = v, vaultId = it.vaultId ?: v.firstOrNull()?.id) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun update(transform: (ImportKeyState) -> ImportKeyState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    /** Replace the pasted text and re-run the Rust inspector (debounced while typing). */
    fun setPrivateKey(text: String) {
        _state.update { it.copy(privateKey = text) }
        inspectJob?.cancel()
        if (text.isBlank()) {
            _state.update { it.copy(preview = null, previewError = null) }
            return
        }
        inspectJob = viewModelScope.launch {
            delay(250)
            runCatching { repo.read { inspectPrivateKey(text) } }
                .onSuccess { p ->
                    _state.update { s ->
                        s.copy(
                            preview = p,
                            previewError = null,
                            label = s.label.ifBlank { p.comment.trim() },
                        )
                    }
                }
                .onFailure { e -> _state.update { it.copy(preview = null, previewError = e.userMessage()) } }
        }
    }

    fun import() {
        val s = _state.value
        val vault = s.vaultId ?: return
        if (!s.canSave) return
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching {
                repo.write {
                    importKey(
                        KeyImportDraft(
                            vaultId = vault,
                            label = s.label.trim().ifBlank { s.preview?.let { str(R.string.type_key, keyTypeLabel(it.keyType, it.bits)) } ?: str(R.string.imported_key) },
                            privateKey = s.privateKey,
                            passphrase = s.passphrase.takeIf { it.isNotEmpty() },
                            rememberPassphrase = s.passphrase.isNotEmpty() && s.remember,
                            certificate = s.certificate.trim().takeIf { it.isNotEmpty() },
                        ),
                    )
                }
            }.onSuccess { k -> _state.update { it.copy(working = false, savedId = k.id) } }
                .onFailure { e -> _state.update { it.copy(working = false, error = e.userMessage()) } }
        }
    }
}
