package com.termoso.android.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Fingerprint
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.data.userMessage
import com.termoso.android.ui.components.EmptyState
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.ListRow
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.components.SubScreen
import com.termoso.android.ui.connections.historySubtitle
import com.termoso.android.ui.hosts.ConfirmDialog
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.HistoryItem
import com.termoso.core.IdentityItem
import com.termoso.core.KeyItem
import com.termoso.core.KnownHostItem
import kotlinx.coroutines.launch

/** Trusted server keys; swipe-free removal via the trailing trash icon. */
@Composable
fun KnownHostsScreen(shell: ShellViewModel, onBack: () -> Unit) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<KnownHostItem>>(emptyList()) }
    var confirm by remember { mutableStateOf<KnownHostItem?>(null) }
    LaunchedEffect(revision) {
        runCatching { shell.repo.read { knownHosts() } }
            .onSuccess { items = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen("Known hosts", onBack) { padding ->
        if (items.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = "No known hosts",
                    hint = "Server keys you accept when connecting are remembered here.",
                    icon = Icons.Filled.Fingerprint,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    items.forEachIndexed { i, k ->
                        if (i > 0) RowDivider()
                        ListRow(
                            title = k.hostname,
                            subtitle = "${k.keyType} · ${k.fingerprint}",
                            leading = { IconTile(Icons.Filled.Fingerprint) },
                        ) {
                            IconButton(onClick = { confirm = k }) {
                                Icon(Icons.Filled.Delete, contentDescription = "Forget", tint = MaterialTheme.colorScheme.onSurfaceVariant)
                            }
                        }
                    }
                }
            }
        }
    }
    confirm?.let { k ->
        ConfirmDialog(
            title = "Forget ${k.hostname}?",
            text = "You will be asked to confirm its key fingerprint on the next connection.",
            confirm = "Forget",
            onConfirm = {
                confirm = null
                scope.launch {
                    runCatching { shell.repo.write { forgetKnownHost(k.id) } }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { confirm = null },
        )
    }
}

/** Past connections, newest first. */
@Composable
fun HistoryScreen(shell: ShellViewModel, onBack: () -> Unit, onOpenHost: (String) -> Unit) {
    val revision by shell.repo.revision.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var items by remember { mutableStateOf<List<HistoryItem>>(emptyList()) }
    var confirmClear by remember { mutableStateOf(false) }
    LaunchedEffect(revision) {
        runCatching { shell.repo.read { history(200u) } }
            .onSuccess { items = it }
            .onFailure { shell.notify(it.userMessage()) }
    }

    SubScreen(
        "History",
        onBack,
        actions = {
            if (items.isNotEmpty()) {
                IconButton(onClick = { confirmClear = true }) { Icon(Icons.Filled.Delete, contentDescription = "Clear history") }
            }
        },
    ) { padding ->
        if (items.isEmpty()) {
            Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.Center) {
                EmptyState(
                    title = "No connections yet",
                    hint = "Sessions you open show up here with their duration.",
                    icon = Icons.Filled.History,
                )
            }
            return@SubScreen
        }
        LazyColumn(Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp)) {
            item {
                SectionCard {
                    items.forEachIndexed { i, h ->
                        if (i > 0) RowDivider()
                        val hostId = h.hostId
                        ListRow(
                            title = h.label.ifBlank { h.target },
                            subtitle = historySubtitle(h),
                            leading = { IconTile(Icons.Filled.History) },
                            modifier = if (hostId != null) Modifier.clickable { onOpenHost(hostId) } else Modifier,
                            titleColor = if (h.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
        }
    }
    if (confirmClear) {
        ConfirmDialog(
            title = "Clear history?",
            text = "Removes all ${items.size} entries from this device.",
            confirm = "Clear",
            onConfirm = {
                confirmClear = false
                scope.launch {
                    runCatching { shell.repo.write { clearHistory() } }
                        .onFailure { shell.notify(it.userMessage()) }
                }
            },
            onDismiss = { confirmClear = false },
        )
    }
}
