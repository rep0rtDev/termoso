package com.termoso.android.ui.snippets

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Code
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.FormField
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.SnippetItem
import com.termoso.core.SnippetPackageItem

/**
 * The `{}` key of the terminal panel: every snippet from every unlocked vault,
 * filtered as you type; picking one opens [RunSnippetDialog] aimed at the
 * current session.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SnippetPickerSheet(
    shell: ShellViewModel,
    sessionId: String,
    onOpenSnippets: () -> Unit,
    onClose: () -> Unit,
) {
    var snippets by remember { mutableStateOf<List<SnippetItem>?>(null) }
    var packages by remember { mutableStateOf<List<SnippetPackageItem>>(emptyList()) }
    var query by remember { mutableStateOf("") }
    var run by remember { mutableStateOf<SnippetItem?>(null) }

    LaunchedEffect(Unit) {
        runCatching { shell.repo.read { snippets(null) to snippetPackages(null) } }
            .onSuccess { (s, p) -> snippets = s; packages = p }
            .onFailure { snippets = emptyList() }
    }

    val shown = snippets.orEmpty().filter { s ->
        query.isBlank() || s.label.contains(query, ignoreCase = true) || s.script.contains(query, ignoreCase = true)
    }

    if (run == null) {
        ModalBottomSheet(onDismissRequest = onClose) {
            Row(
                Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("Snippets", style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                TextButton(onClick = { onClose(); onOpenSnippets() }) { Text("Manage") }
            }
            if (snippets.orEmpty().size > 5) {
                Row(Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                    FormField(query, { query = it }, "Search")
                }
            }
            when {
                snippets == null -> Spacer(Modifier.height(120.dp))
                shown.isEmpty() -> {
                    EmptyState(
                        title = if (query.isBlank()) "No snippets yet" else "Nothing matches",
                        hint = if (query.isBlank()) "Create one under Vaults → Snippets." else "Try another name or command.",
                        modifier = Modifier.padding(16.dp),
                    )
                    Spacer(Modifier.height(24.dp))
                }
                else -> LazyColumn(contentPadding = PaddingValues(bottom = 32.dp)) {
                    items(shown, key = { it.id }) { s ->
                        val pkg = packages.firstOrNull { it.id == s.packageId }?.label
                        ListRow(
                            title = s.label,
                            subtitle = listOfNotNull(pkg, snippetSubtitle(s)).joinToString(" · "),
                            leading = { IconTile(Icons.Filled.Code) },
                            modifier = Modifier.clickable { run = s },
                        )
                    }
                }
            }
        }
    }

    run?.let { s ->
        RunSnippetDialog(
            shell = shell,
            snippet = s,
            preselect = listOf(sessionId),
            onDismiss = { run = null },
            onLaunched = { onClose() },
        )
    }
}
