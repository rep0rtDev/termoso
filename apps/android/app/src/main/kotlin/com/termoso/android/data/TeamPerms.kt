package com.termoso.android.data

import com.termoso.core.TeamCard
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Team permissions on the client. They are an attribute of a *team*, so they
 * only restrict actions on objects in that team's vaults — personal and local
 * vaults are never touched. Permissions without a vault (local terminal,
 * serial, multiplayer start) follow strictest-team-wins like policies.
 *
 * Marked `enforced_by: client` in the server catalog: UX for managed teams,
 * not a security boundary — the server never sees vault contents.
 */
class TeamPerms(private val repo: VaultRepository) {
    private val _teams = MutableStateFlow<List<TeamCard>>(emptyList())
    val teams: StateFlow<List<TeamCard>> = _teams.asStateFlow()

    /** Reloads the team list; failures keep the previous snapshot. */
    suspend fun refresh() {
        runCatching { repo.read { teams() } }.onSuccess { _teams.value = it }
    }

    fun clear() { _teams.value = emptyList() }

    /** Team owning [vaultId], or null for personal/local vaults. */
    suspend fun vaultTeam(vaultId: String?): TeamCard? {
        if (vaultId == null || _teams.value.isEmpty()) return null
        val teamId = repo.read { vaults().firstOrNull { it.id == vaultId }?.teamId } ?: return null
        return _teams.value.firstOrNull { it.id == teamId }
    }

    /** Allowed for an object in [vaultId]; always true outside team vaults. */
    suspend fun canInVault(vaultId: String?, permission: String): Boolean =
        vaultTeam(vaultId)?.can(permission) ?: true

    /** Every team the user belongs to must grant it. */
    fun canAnywhere(permission: String): Boolean = _teams.value.all { it.can(permission) }

    /** Throws [PermissionDenied] when the team role lacks [permission]. */
    suspend fun require(vaultId: String?, permission: String) {
        val team = vaultTeam(vaultId) ?: return
        if (!team.can(permission)) throw PermissionDenied(permission, team.name)
    }

    fun requireAnywhere(permission: String) {
        val team = _teams.value.firstOrNull { !it.can(permission) } ?: return
        throw PermissionDenied(permission, team.name)
    }
}

/** Holds [permission] in this team; owners carry `*`. 0.8 servers send no list — fall back to the coarse role. */
fun TeamCard.can(permission: String): Boolean =
    if (myPermissions.isEmpty()) myRole == com.termoso.core.TeamRole.ADMIN || myRole == com.termoso.core.TeamRole.OWNER
    else myPermissions.any { it == "*" || it == permission }

/** Shown as-is by [Throwable.userMessage]. */
class PermissionDenied(val permission: String, val team: String) :
    IllegalStateException("Your role in $team does not include \u201c$permission\u201d.")
