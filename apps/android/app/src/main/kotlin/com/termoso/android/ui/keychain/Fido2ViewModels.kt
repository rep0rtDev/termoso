package com.termoso.android.ui.keychain

import androidx.annotation.StringRes
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.R
import com.termoso.android.data.Fido2Manager
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.Fido2DeviceCard
import com.termoso.core.Fido2GenerateDraft
import com.termoso.core.Fido2Listener
import com.termoso.core.Fido2LoadDraft
import com.termoso.core.MobileException
import com.termoso.core.SkKeyAlgorithm
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Security-key algorithms, in display order. */
enum class SkKind(val label: String, @StringRes val hint: Int, val algorithm: SkKeyAlgorithm) {
    ED25519("Ed25519", R.string.sk_kind_ed25519_hint, SkKeyAlgorithm.ED25519),
    ECDSA("ECDSA P-256", R.string.sk_kind_ecdsa_hint, SkKeyAlgorithm.ECDSA_P256),
}

data class Fido2GenerateState(
    val vaults: List<VaultInfo> = emptyList(),
    val vaultId: String? = null,
    val deviceId: String? = null,
    val label: String = "",
    val kind: SkKind = SkKind.ED25519,
    val application: String = "ssh:",
    val resident: Boolean = false,
    val userPresence: Boolean = true,
    val userVerification: Boolean = false,
    val pin: String = "",
    val user: String = "",
    val comment: String = "",
    val passphrase: String = "",
    val confirm: String = "",
    val remember: Boolean = true,
    val working: Boolean = false,
    val touch: Boolean = false,
    val savedId: String? = null,
    val error: String? = null,
) {
    val passphraseMismatch get() = passphrase.isNotEmpty() && confirm.isNotEmpty() && passphrase != confirm

    /** Whether the chosen (or only) token can do what the form asks. */
    fun device(devices: List<Fido2DeviceCard>): Fido2DeviceCard? =
        devices.firstOrNull { it.id == deviceId } ?: devices.singleOrNull()

    fun canSave(devices: List<Fido2DeviceCard>): Boolean {
        val d = device(devices) ?: return false
        if (working || vaultId == null || passphraseMismatch) return false
        if (kind == SkKind.ED25519 && !d.ed25519) return false
        if (resident && !d.residentKeys) return false
        if (application.isBlank()) return false
        return true
    }
}

/**
 * Drives `fido2_generate`: Rust talks to the token through the registry, this
 * only carries the form and mirrors the "touch now" moment. The PIN goes to
 * Rust for the duration of the call and is cleared from the form afterwards.
 */
class Fido2GenerateViewModel(
    private val repo: VaultRepository,
    val fido2: Fido2Manager,
    initialVault: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(Fido2GenerateState(vaultId = initialVault))
    val state: StateFlow<Fido2GenerateState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            runCatching { repo.read { vaults().filter { !it.locked } } }
                .onSuccess { v -> _state.update { it.copy(vaults = v, vaultId = it.vaultId ?: v.firstOrNull()?.id) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun update(transform: (Fido2GenerateState) -> Fido2GenerateState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    fun generate() {
        val s = _state.value
        val vault = s.vaultId ?: return
        val devices = fido2.devices.value
        if (!s.canSave(devices)) return
        val device = s.device(devices) ?: return
        _state.update { it.copy(working = true, touch = false) }
        val listener = object : Fido2Listener {
            override fun onTouch() = _state.update { it.copy(touch = true) }
        }
        viewModelScope.launch {
            runCatching {
                repo.write {
                    fido2Generate(
                        Fido2GenerateDraft(
                            vaultId = vault,
                            deviceId = device.id,
                            label = s.label.trim().ifBlank { str(R.string.security_key_2, s.kind.label) },
                            algorithm = s.kind.algorithm,
                            application = s.application.trim(),
                            resident = s.resident,
                            userPresence = s.userPresence,
                            userVerification = s.userVerification,
                            pin = s.pin.takeIf { it.isNotEmpty() },
                            user = s.user.trim(),
                            comment = s.comment.trim(),
                            passphrase = s.passphrase.takeIf { it.isNotEmpty() },
                            rememberPassphrase = s.passphrase.isNotEmpty() && s.remember,
                        ),
                        listener,
                    )
                }
            }.onSuccess { k -> _state.update { it.copy(working = false, touch = false, pin = "", savedId = k.id) } }
                .onFailure { e -> _state.update { it.copy(working = false, touch = false, pin = keepPin(e, it.pin), error = e.userMessage()) } }
        }
    }
}

data class Fido2LoadState(
    val vaults: List<VaultInfo> = emptyList(),
    val vaultId: String? = null,
    val deviceId: String? = null,
    val pin: String = "",
    val passphrase: String = "",
    val confirm: String = "",
    val remember: Boolean = true,
    val working: Boolean = false,
    val touch: Boolean = false,
    val loaded: List<String>? = null,
    val error: String? = null,
) {
    val passphraseMismatch get() = passphrase.isNotEmpty() && confirm.isNotEmpty() && passphrase != confirm

    fun device(devices: List<Fido2DeviceCard>): Fido2DeviceCard? =
        devices.firstOrNull { it.id == deviceId } ?: devices.singleOrNull()

    fun canSave(devices: List<Fido2DeviceCard>): Boolean =
        !working && vaultId != null && device(devices) != null && pin.isNotEmpty() && !passphraseMismatch
}

/** Drives `fido2_load_resident`: pulls every `ssh:` resident credential off a token into the vault. */
class Fido2LoadViewModel(
    private val repo: VaultRepository,
    val fido2: Fido2Manager,
    initialVault: String?,
) : ViewModel() {
    private val _state = MutableStateFlow(Fido2LoadState(vaultId = initialVault))
    val state: StateFlow<Fido2LoadState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            runCatching { repo.read { vaults().filter { !it.locked } } }
                .onSuccess { v -> _state.update { it.copy(vaults = v, vaultId = it.vaultId ?: v.firstOrNull()?.id) } }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun update(transform: (Fido2LoadState) -> Fido2LoadState) = _state.update(transform)

    fun errorShown() = _state.update { it.copy(error = null) }

    fun load() {
        val s = _state.value
        val vault = s.vaultId ?: return
        val devices = fido2.devices.value
        if (!s.canSave(devices)) return
        val device = s.device(devices) ?: return
        _state.update { it.copy(working = true, touch = false) }
        val listener = object : Fido2Listener {
            override fun onTouch() = _state.update { it.copy(touch = true) }
        }
        viewModelScope.launch {
            runCatching {
                repo.write {
                    fido2LoadResident(
                        Fido2LoadDraft(
                            vaultId = vault,
                            deviceId = device.id,
                            pin = s.pin,
                            passphrase = s.passphrase.takeIf { it.isNotEmpty() },
                            rememberPassphrase = s.passphrase.isNotEmpty() && s.remember,
                        ),
                        listener,
                    )
                }
            }.onSuccess { keys -> _state.update { it.copy(working = false, touch = false, pin = "", loaded = keys.map { k -> k.id }) } }
                .onFailure { e -> _state.update { it.copy(working = false, touch = false, pin = keepPin(e, it.pin), error = e.userMessage()) } }
        }
    }
}

/** A wrong PIN is cleared so it cannot be resent by accident; any other failure keeps the form intact. */
private fun keepPin(e: Throwable, pin: String): String =
    if (e is MobileException.SecurityKey && e.kind.startsWith("fido2_pin")) "" else pin
