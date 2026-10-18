package com.termoso.android.ui.team

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.R
import com.termoso.android.data.AccountManager
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.InviteCard
import com.termoso.core.InviteSent
import com.termoso.core.PendingKeyCard
import com.termoso.core.TeamCard
import com.termoso.core.MobileException
import com.termoso.core.TeamMemberCard
import com.termoso.core.TeamRole
import com.termoso.core.VaultAccess
import com.termoso.core.VaultAccessDraft
import com.termoso.core.VaultInfo
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

val TeamRole.isAdmin: Boolean get() = this == TeamRole.ADMIN || this == TeamRole.OWNER

/** Team policy `mfa` blocks the request until the account enrols a second factor. */
fun Throwable.isMfaRequired(): Boolean = this is MobileException.Other && kind == "mfa_required"

/** Holds [permission] in this team; owners carry `*`. 0.8 servers send no list — fall back to the coarse role. */
fun TeamCard.can(permission: String): Boolean =
    if (myPermissions.isEmpty()) myRole.isAdmin else myPermissions.any { it == "*" || it == permission }

/** Can do at least one of invite / change role / remove — enough to show the admin blocks. */
val TeamCard.canManageMembers: Boolean
    get() = can("members.invite") || can("members.role") || can("members.remove")

fun TeamRole.label(): String = when (this) {
    TeamRole.MEMBER -> str(R.string.member)
    TeamRole.ADMIN -> str(R.string.admin)
    TeamRole.OWNER -> str(R.string.owner)
}

fun TeamRole.hint(): String = when (this) {
    TeamRole.MEMBER -> str(R.string.uses_the_vaults_they_were_given_access_to)
    TeamRole.ADMIN -> str(R.string.also_invites_removes_members_and_creates_vaults)
    TeamRole.OWNER -> str(R.string.everything_including_deleting_the_team)
}

fun VaultAccess.label(): String = when (this) {
    VaultAccess.VIEW -> str(R.string.can_view)
    VaultAccess.EDIT -> str(R.string.can_edit)
    VaultAccess.MANAGE -> str(R.string.can_manage)
}

fun VaultAccess.hint(): String = when (this) {
    VaultAccess.VIEW -> str(R.string.connects_and_reads_hosts_keys_and_snippets)
    VaultAccess.EDIT -> str(R.string.also_adds_changes_and_removes_items)
    VaultAccess.MANAGE -> str(R.string.also_decides_who_has_access_and_rotates_the)
}

/** Splits a pasted list of addresses (commas, semicolons, spaces, new lines), lower-cased, de-duplicated. */
fun splitEmails(text: String): List<String> =
    text.split(Regex("[\\s,;]+")).map { it.trim().lowercase() }.filter { it.isNotEmpty() }.distinct()

fun looksLikeEmail(e: String): Boolean = Regex("^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$").matches(e)

data class TeamsUiState(
    val loading: Boolean = true,
    val teams: List<TeamCard> = emptyList(),
    val error: String? = null,
)

/** Settings → Account → Teams: the teams we belong to, create, join by link. */
class TeamsViewModel(private val repo: VaultRepository) : ViewModel() {
    private val _state = MutableStateFlow(TeamsUiState())
    val state: StateFlow<TeamsUiState> = _state.asStateFlow()

    /** Called every time the screen is shown, so returning from a team reflects rename / leave / delete. */
    fun reload() {
        viewModelScope.launch {
            runCatching { repo.read { teams() } }
                .onSuccess { t -> _state.update { it.copy(loading = false, teams = t, error = null) } }
                .onFailure { e -> _state.update { it.copy(loading = false, error = e.userMessage()) } }
        }
    }

    suspend fun create(name: String): TeamCard = repo.write { createTeam(name) }.also { reload() }

    suspend fun join(link: String): TeamCard = repo.write { acceptTeamInvite(link) }.also { reload() }
}

data class TeamUiState(
    val loading: Boolean = true,
    val team: TeamCard? = null,
    val members: List<TeamMemberCard> = emptyList(),
    val invites: List<InviteCard> = emptyList(),
    val pendingKeys: List<PendingKeyCard> = emptyList(),
    val vaults: List<VaultInfo> = emptyList(),
    val error: String? = null,
    /** The server refused team data until this account turns on two-factor authentication (team policy). */
    val mfaRequired: Boolean = false,
) {
    val me: TeamMemberCard? get() = members.firstOrNull { it.me }
    val isAdmin: Boolean get() = team?.canManageMembers == true
    val isOwner: Boolean get() = team?.myRole == TeamRole.OWNER
}

/**
 * One team: members, invitations, vaults and keys waiting to be granted.
 * Everything is fetched from the server through Rust on demand; the vault
 * list comes from the local store (refreshed by Rust after each change).
 */
class TeamViewModel(
    private val repo: VaultRepository,
    private val account: AccountManager,
    private val teamId: String,
) : ViewModel() {
    private val _state = MutableStateFlow(TeamUiState())
    val state: StateFlow<TeamUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            account.status.collect { s ->
                _state.update { it.copy(vaults = s.vaults.filter { v -> v.teamId == teamId }) }
            }
        }
    }

    fun reload() {
        viewModelScope.launch {
            val team = runCatching { repo.read { teams().firstOrNull { it.id == teamId } } }
                .getOrElse { e ->
                    _state.update { it.copy(loading = false, mfaRequired = e.isMfaRequired(), error = e.userMessage().takeUnless { _ -> e.isMfaRequired() }) }
                    return@launch
                }
            if (team == null) {
                _state.update { it.copy(loading = false, team = null, error = str(R.string.you_are_no_longer_in_this_team)) }
                return@launch
            }
            val admin = team.canManageMembers
            runCatching {
                repo.read {
                    Triple(
                        teamMembers(teamId),
                        if (admin) teamInvites(teamId) else emptyList(),
                        if (admin) teamPendingKeys(teamId) else emptyList(),
                    )
                }
            }.onSuccess { (members, invites, pending) ->
                _state.update {
                    it.copy(loading = false, team = team, members = members, invites = invites, pendingKeys = pending, error = null)
                }
            }.onFailure { e ->
                _state.update { it.copy(loading = false, team = team, mfaRequired = e.isMfaRequired(), error = e.userMessage().takeUnless { _ -> e.isMfaRequired() }) }
            }
        }
    }

    fun errorShown() = _state.update { it.copy(error = null) }

    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch {
            runCatching { block() }
                .onSuccess { reload() }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
        }
    }

    fun rename(name: String) = mutate { repo.write { renameTeam(teamId, name) } }

    fun setMultiplayer(on: Boolean) = mutate { repo.read { setTeamSecurity(teamId, on, null, null) } }

    fun setRequireMfa(on: Boolean) = mutate { repo.read { setTeamSecurity(teamId, null, on, null) } }

    fun setPresence(on: Boolean) = mutate { repo.read { setTeamSecurity(teamId, null, null, on) } }

    fun setRole(userId: String, role: TeamRole) = mutate { repo.read { setTeamMemberRole(teamId, userId, role) } }

    fun removeMember(userId: String) = mutate { repo.write { removeTeamMember(teamId, userId) } }

    fun revokeInvite(inviteId: String) = mutate { repo.read { revokeTeamInvite(teamId, inviteId) } }

    fun grantKey(p: PendingKeyCard) = mutate { repo.read { setTeamVaultAccess(p.vaultId, p.userId, p.access) } }

    suspend fun invite(emails: List<String>, role: TeamRole, vaultIds: List<String>): List<InviteSent> =
        repo.read { teamInvite(teamId, emails, role, vaultIds) }.also { reload() }

    suspend fun createVault(name: String, access: Map<String, VaultAccess>) {
        repo.write {
            createTeamVault(teamId, name, access.map { (u, a) -> VaultAccessDraft(u, a) })
        }
        reload()
    }

    suspend fun delete() = repo.write { deleteTeam(teamId) }

    suspend fun leave() = repo.write { leaveTeam(teamId) }
}
