package com.termoso.android.ui.hosts

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.userMessage
import com.termoso.android.data.VaultRepository
import com.termoso.core.EnvVar
import com.termoso.core.GroupItem
import com.termoso.core.HostDraft
import com.termoso.core.IdentityItem
import com.termoso.core.InheritedInfo
import com.termoso.core.KeyItem
import com.termoso.core.QuickTarget
import com.termoso.core.SnippetItem
import com.termoso.core.TagItem
import com.termoso.core.TelnetDraft
import com.termoso.core.VaultInfo
import com.termoso.core.WebDavDraft
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** [WebDavDraft.auth] values, as the store spells them. */
const val WEBDAV_AUTH_PASSWORD = "password"
const val WEBDAV_AUTH_TOKEN = "token"

/** A blank WebDAV section for "+ Add WebDAV". */
fun emptyWebDavDraft() = WebDavDraft(
    url = "",
    username = "",
    password = null,
    identityId = null,
    certificateFingerprint = null,
    hasPassword = false,
    auth = WEBDAV_AUTH_PASSWORD,
    bearerToken = null,
    hasBearerToken = false,
    clientCertificate = null,
    clientKey = null,
    clientCertificateFingerprint = null,
)

/**
 * A fresh draft started from a quick-connect target ("Add to hosts" on an ad-hoc
 * terminal): address, user and port land in the matching protocol section; a
 * Telnet target gets a Telnet section instead of the SSH one.
 */
fun HostDraft.prefilled(target: QuickTarget?): HostDraft {
    if (target == null) return this
    return if (target.protocol.equals("telnet", true)) {
        copy(
            address = target.host,
            ssh = false,
            telnet = TelnetDraft(
                port = target.port.takeIf { it != 23.toUShort() },
                username = target.username,
                password = null,
                identityId = null,
                hasPassword = false,
            ),
        )
    } else {
        copy(
            address = target.host,
            username = target.username,
            port = target.port.takeIf { it != 22.toUShort() },
        )
    }
}

data class HostEditorState(
    val loading: Boolean = true,
    val draft: HostDraft? = null,
    val vaults: List<VaultInfo> = emptyList(),
    val groups: List<GroupItem> = emptyList(),
    val tags: List<TagItem> = emptyList(),
    val keys: List<KeyItem> = emptyList(),
    val identities: List<IdentityItem> = emptyList(),
    val snippets: List<SnippetItem> = emptyList(),
    val inherited: InheritedInfo? = null,
    val saving: Boolean = false,
    /** The form is finished (saved, duplicated or removed) and should close. */
    val saved: Boolean = false,
    /** Id of the host just written by [HostEditorViewModel.save], for "save, then connect" flows. */
    val savedId: String? = null,
    val error: String? = null,
    /** Leaf SHA-256 of the WebDAV client certificate typed into the form, once it validated. */
    val clientCertificateFingerprint: String? = null,
    /** Why the typed WebDAV client certificate / key pair does not validate. */
    val clientCertificateError: String? = null,
) {
    val canSave: Boolean
        get() {
            val d = draft ?: return false
            if (saving) return false
            // A WebDAV-only host takes its address from the share URL.
            val webdavOnly = !d.ssh && d.telnet == null && d.webdav != null
            return d.address.isNotBlank() || (webdavOnly && d.webdav?.url?.isNotBlank() == true)
        }
}

/**
 * New/edit host form over [HostDraft]. The draft is the only thing that holds a
 * password, and only when the user typed one; `null` keeps the stored secret.
 */
class HostEditorViewModel(
    private val repo: VaultRepository,
    private val hostId: String?,
    private val initialVault: String?,
    private val groupId: String?,
    /** For a new host: the quick-connect target (`user@host:port`, `telnet://host`) to start the form from. */
    private val prefill: QuickTarget? = null,
) : ViewModel() {
    private val _state = MutableStateFlow(HostEditorState())
    val state: StateFlow<HostEditorState> = _state.asStateFlow()

    init {
        viewModelScope.launch { load() }
    }

    private suspend fun load() {
        runCatching {
            repo.read {
                val vaults = vaults().filter { !it.locked }
                val draft = if (hostId != null) {
                    hostDraft(hostId)
                } else {
                    val vault = initialVault ?: vaults.first().id
                    newHostDraft(vault, groupId).prefilled(prefill)
                }
                HostEditorState(
                    loading = false,
                    draft = draft,
                    vaults = vaults,
                    groups = groups(draft.vaultId),
                    tags = tags(draft.vaultId),
                    keys = keys(draft.vaultId),
                    identities = identities(draft.vaultId),
                    snippets = snippets(draft.vaultId),
                    inherited = inherited(draft.groupId),
                )
            }
        }.onSuccess { s -> _state.value = s }
            .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
    }

    fun update(transform: (HostDraft) -> HostDraft) {
        _state.update { s -> s.draft?.let { s.copy(draft = transform(it)) } ?: s }
    }

    /** Edit the Telnet section; no-op while the host has none. */
    fun updateTelnet(transform: (TelnetDraft) -> TelnetDraft) = update { d ->
        d.telnet?.let { d.copy(telnet = transform(it)) } ?: d
    }

    /** Edit the WebDAV section; no-op while the host has none. */
    fun updateWebdav(transform: (WebDavDraft) -> WebDavDraft) {
        update { d -> d.webdav?.let { d.copy(webdav = transform(it)) } ?: d }
        validateClientCertificate()
    }

    /**
     * Split a pasted / picked PEM file into the client certificate and key
     * fields; a part the file does not contain leaves the field alone.
     */
    fun importClientPem(text: String) {
        viewModelScope.launch {
            runCatching { repo.read { splitClientPem(text) } }
                .onSuccess { parts ->
                    updateWebdav {
                        it.copy(
                            clientCertificate = parts.certificate.ifBlank { it.clientCertificate ?: "" },
                            clientKey = parts.privateKey.ifBlank { it.clientKey ?: "" },
                        )
                    }
                }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    private var certificateCheck: Job? = null

    private fun validateClientCertificate() {
        val w = _state.value.draft?.webdav
        val cert = w?.clientCertificate?.trim().orEmpty()
        val key = w?.clientKey?.trim().orEmpty()
        certificateCheck?.cancel()
        if (cert.isEmpty() || key.isEmpty()) {
            _state.update { it.copy(clientCertificateFingerprint = null, clientCertificateError = null) }
            return
        }
        certificateCheck = viewModelScope.launch {
            val result = runCatching { repo.read { inspectClientCertificate(cert, key) } }
            _state.update {
                it.copy(
                    clientCertificateFingerprint = result.getOrNull(),
                    clientCertificateError = result.exceptionOrNull()?.userMessage(),
                )
            }
        }
    }

    /** Switching vault (new hosts only) resets group/tags/key/identity, which are per-vault. */
    fun setVault(vaultId: String) {
        val current = _state.value.draft ?: return
        if (current.vaultId == vaultId || current.id != null) return
        viewModelScope.launch {
            runCatching {
                repo.read {
                    _state.value.copy(
                        draft = current.copy(
                            vaultId = vaultId,
                            groupId = null,
                            tagIds = emptyList(),
                            sshKeyId = null,
                            identityId = null,
                            startupSnippetId = null,
                        ),
                        groups = groups(vaultId),
                        tags = tags(vaultId),
                        keys = keys(vaultId),
                        identities = identities(vaultId),
                        snippets = snippets(vaultId),
                        inherited = inherited(null),
                    )
                }
            }.onSuccess { _state.value = it }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun setGroup(groupId: String?) {
        update { it.copy(groupId = groupId) }
        viewModelScope.launch {
            runCatching { repo.read { inherited(groupId) } }
                .onSuccess { inh -> _state.update { it.copy(inherited = inh) } }
        }
    }

    fun toggleTag(tagId: String) = update {
        it.copy(tagIds = if (tagId in it.tagIds) it.tagIds - tagId else it.tagIds + tagId)
    }

    fun createTag(label: String) {
        val draft = _state.value.draft ?: return
        viewModelScope.launch {
            runCatching { repo.write { createTag(draft.vaultId, label.trim()) } }
                .onSuccess { tag ->
                    _state.update { s ->
                        s.copy(
                            tags = (s.tags + tag).distinctBy { it.id },
                            draft = s.draft?.copy(tagIds = s.draft.tagIds + tag.id),
                        )
                    }
                }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun setEnv(index: Int, name: String, value: String) = update {
        it.copy(envVariables = it.envVariables.toMutableList().apply { this[index] = EnvVar(name, value) })
    }

    fun addEnv() = update { it.copy(envVariables = it.envVariables + EnvVar("", "")) }

    fun removeEnv(index: Int) = update {
        it.copy(envVariables = it.envVariables.toMutableList().apply { removeAt(index) })
    }

    fun save() {
        val draft = _state.value.draft ?: return
        if (!_state.value.canSave) return
        _state.update { it.copy(saving = true) }
        viewModelScope.launch {
            val clean = draft.copy(
                label = draft.label.trim(),
                address = draft.address.trim(),
                username = draft.username.trim(),
                telnet = draft.telnet?.let { it.copy(username = it.username.trim()) },
                webdav = draft.webdav?.let {
                    val tokenMode = it.auth == WEBDAV_AUTH_TOKEN
                    it.copy(
                        url = it.url.trim(),
                        username = if (tokenMode) "" else it.username.trim(),
                        // Switching modes drops the other mode's secret.
                        password = if (tokenMode) "" else it.password,
                        bearerToken = if (tokenMode) it.bearerToken else "",
                        certificateFingerprint = it.certificateFingerprint?.trim()?.ifEmpty { null },
                        // A new certificate with an empty key field keeps the stored key (renewal).
                        clientKey = if (it.clientCertificateFingerprint != null) it.clientKey?.ifBlank { null } else it.clientKey,
                    )
                },
                envVariables = draft.envVariables.filter { it.name.isNotBlank() },
            )
            runCatching { repo.write { saveHost(clean) } }
                .onSuccess { host -> _state.update { it.copy(saving = false, saved = true, savedId = host.id) } }
                .onFailure { e -> _state.update { it.copy(saving = false, error = e.userMessage()) } }
        }
    }

    /** Existing hosts only: a copy next to the original, then the form closes. */
    fun duplicate() = finishWith { id -> repo.write { duplicateHost(id) } }

    /** Existing hosts only: remove the host, then the form closes. */
    fun delete() = finishWith { id -> repo.write { deleteHosts(listOf(id)) } }

    private fun finishWith(block: suspend (String) -> Unit) {
        val id = _state.value.draft?.id ?: return
        _state.update { it.copy(saving = true) }
        viewModelScope.launch {
            runCatching { block(id) }
                .onSuccess { _state.update { it.copy(saving = false, saved = true) } }
                .onFailure { e -> _state.update { it.copy(saving = false, error = e.userMessage()) } }
        }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    fun showError(message: String) = _state.update { it.copy(error = message) }
}
