package com.termoso.android.ui.team

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.AuditEventCard
import kotlinx.coroutines.launch

private const val PAGE = 50u

/** Team activity log, newest first, paged by "Load more" like the desktop and the web cabinet. */
@Composable
fun TeamActivityScreen(shell: ShellViewModel, teamId: String, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    var events by remember { mutableStateOf<List<AuditEventCard>?>(null) }
    var nextBefore by remember { mutableStateOf<Long?>(null) }
    var loading by remember { mutableStateOf(false) }

    suspend fun load(before: Long?) {
        loading = true
        runCatching { shell.repo.read { teamAudit(teamId, before, PAGE) } }
            .onSuccess { page ->
                events = (events ?: emptyList()) + page.events
                nextBefore = page.nextBefore
            }
            .onFailure { shell.notify(it.userMessage()) }
        loading = false
    }
    LaunchedEffect(teamId) { load(null) }

    SubScreen(title = "Activity log", onBack = onBack) { padding ->
        val list = events
        when {
            list == null -> Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
            }
            list.isEmpty() -> Box(Modifier.fillMaxSize().padding(padding)) {
                EmptyState(title = "Nothing yet", hint = "Joins, invitations, access changes and key rotations show up here.")
            }
            else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(top = padding.calculateTopPadding(), bottom = 24.dp)) {
                items(list, key = { it.id }) { e ->
                    EventRow(e)
                    HorizontalDivider(Modifier.padding(start = 16.dp), color = MaterialTheme.colorScheme.outlineVariant)
                }
                if (nextBefore != null) {
                    item {
                        Box(Modifier.fillMaxWidth().padding(12.dp), contentAlignment = Alignment.Center) {
                            if (loading) {
                                CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
                            } else {
                                TextButton(onClick = { scope.launch { load(nextBefore) } }) { Text("Load more") }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun EventRow(e: AuditEventCard) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp)) {
        Text(actionLabel(e.action), style = MaterialTheme.typography.bodyLarge)
        val who = buildList {
            e.actor?.let { add(it) }
            e.target?.let { add("→ $it") }
        }.joinToString(" ")
        if (who.isNotEmpty()) {
            Text(who, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        e.details.forEach { d ->
            Text(d, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Text(
            e.createdAt.replace('T', ' ').take(16),
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** `team.member_removed` → "Member removed". */
private fun actionLabel(action: String): String {
    val tail = action.substringAfterLast('.').replace('_', ' ')
    return tail.replaceFirstChar { it.uppercase() }
}
