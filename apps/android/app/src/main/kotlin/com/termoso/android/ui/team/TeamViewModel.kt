package com.termoso.android.ui.team

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.termoso.android.data.AccountManager
import com.termoso.android.data.VaultRepository
import com.termoso.android.data.userMessage
import com.termoso.core.InviteCard
import com.termoso.core.InviteSent
import com.termoso.core.PendingKeyCard
import com.termoso.core.TeamCard
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

fun TeamRole.label(): String = when (this) {
    TeamRole.MEMBER -> "Member"
    TeamRole.ADMIN -> "Admin"
    TeamRole.OWNER -> "Owner"
}

fun TeamRole.hint(): String = when (this) {
    TeamRole.MEMBER -> "Uses the vaults they were given access to"
    TeamRole.ADMIN -> "Also invites, removes members and creates vaults"
    TeamRole.OWNER -> "Everything, including deleting the team"
}

fun VaultAccess.label(): String = when (this) {
    VaultAccess.VIEW -> "can view"
    VaultAccess.EDIT -> "can edit"
    VaultAccess.MANAGE -> "can manage"
}

fun VaultAccess.hint(): String = when (this) {
    VaultAccess.VIEW -> "Connects and reads hosts, keys and snippets"
    VaultAccess.EDIT -> "Also adds, changes and removes items"
    VaultAccess.MANAGE -> "Also decides who has access and rotates the key"
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
) {
    val me: TeamMemberCard? get() = members.firstOrNull { it.me }
    val isAdmin: Boolean get() = team?.myRole?.isAdmin == true
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
                    _state.update { it.copy(loading = false, error = e.userMessage()) }
                    return@launch
                }
            if (team == null) {
                _state.update { it.copy(loading = false, team = null, error = "You are no longer in this team") }
                return@launch
            }
            val admin = team.myRole.isAdmin
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
                _state.update { it.copy(loading = false, team = team, error = e.userMessage()) }
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

    fun setMultiplayer(on: Boolean) = mutate { repo.read { setTeamSecurity(teamId, on, null) } }

    fun setRequireMfa(on: Boolean) = mutate { repo.read { setTeamSecurity(teamId, null, on) } }

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
