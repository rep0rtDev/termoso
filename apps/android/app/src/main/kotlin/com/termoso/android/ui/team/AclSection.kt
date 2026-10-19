package com.termoso.android.ui.team

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.AclRuleCard

/** Read-only list of the vault's group/host access rules, resolved to local names.
 *  Rules are edited in the web cabinet and the desktop app. */
@Composable
fun AclSection(shell: ShellViewModel, teamId: String, vaultId: String) {
    var rules by remember { mutableStateOf<List<AclRuleCard>?>(null) }
    var names by remember { mutableStateOf<Map<String, String>>(emptyMap()) }
    LaunchedEffect(teamId, vaultId) {
        runCatching { shell.repo.read { teamAcl(teamId, vaultId) } }.onSuccess { rules = it }
        runCatching {
            shell.repo.read {
                hosts(vaultId).associate { it.id to it.label } + groups(vaultId).associate { it.id to it.label }
            }
        }.onSuccess { names = it }
    }
    val list = rules ?: return
    SectionLabel(stringResource(R.string.group_and_host_access))
    SectionCard {
        Text(
            stringResource(R.string.acl_hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        )
        if (list.isEmpty()) {
            RowDivider()
            Text(stringResource(R.string.no_acl_rules), modifier = Modifier.padding(16.dp), style = MaterialTheme.typography.bodyMedium)
        }
        list.forEach { r ->
            RowDivider()
            ListRow(
                title = names[r.entityId] ?: r.entityId.take(8),
                subtitle = r.principalName + " · " + stringResource(
                    when (r.mode) {
                        "none" -> R.string.acl_hidden
                        "connect" -> R.string.acl_connect
                        "read" -> R.string.acl_read
                        else -> R.string.acl_edit
                    },
                ),
            )
        }
    }
}
