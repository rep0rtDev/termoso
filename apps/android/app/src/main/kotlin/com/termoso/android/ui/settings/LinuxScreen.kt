package com.termoso.android.ui.settings

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.termoso.android.R
import com.termoso.android.ui.components.ChevronRow
import com.termoso.android.ui.components.IconTile
import com.termoso.android.ui.components.RowDivider
import com.termoso.android.ui.components.SectionCard
import com.termoso.android.ui.components.SectionLabel
import com.termoso.android.ui.shell.ShellViewModel
import com.termoso.core.LinuxStage
import kotlinx.coroutines.launch

/**
 * Settings → Linux environment: pick Alpine or Ubuntu, or bring your own
 * rootfs (file or URL), watch it download and unpack, open a shell in it or
 * remove it. One system at a time.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LinuxScreen(shell: ShellViewModel, onBack: () -> Unit, onOpenTerminal: () -> Unit) {
    val linux = shell.sessions.linux
    val installed by linux.installed.collectAsStateWithLifecycle()
    val progress by linux.progress.collectAsStateWithLifecycle()
    val error by linux.error.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var confirmRemove by remember { mutableStateOf(false) }
    var urlDialog by remember { mutableStateOf(false) }
    val pickFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) scope.launch { linux.installFromUri(uri) }
    }

    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text(stringResource(R.string.remove_linux)) },
            text = { Text(stringResource(R.string.remove_linux_explain)) },
            confirmButton = {
                TextButton(onClick = { confirmRemove = false; scope.launch { linux.remove() } }) {
                    Text(stringResource(R.string.remove_linux))
                }
            },
            dismissButton = { TextButton(onClick = { confirmRemove = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    error?.let { message ->
        AlertDialog(
            onDismissRequest = { linux.clearError() },
            title = { Text(stringResource(R.string.installation_failed)) },
            text = { Text(message) },
            confirmButton = { TextButton(onClick = { linux.clearError() }) { Text(stringResource(android.R.string.ok)) } },
        )
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.linux_environment)) },
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back)) }
                },
            )
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            Text(
                stringResource(R.string.linux_environment_explain),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = 12.dp),
            )
            installed?.let { sys ->
                SectionLabel(stringResource(R.string.installed))
                SectionCard {
                    ChevronRow(
                        title = "${sys.name} ${sys.version}",
                        subtitle = stringResource(R.string.open_linux_shell),
                        leading = { IconTile(Icons.Filled.Terminal) },
                        modifier = Modifier.clickable {
                            scope.launch { if (shell.connectLocal() != null) onOpenTerminal() }
                        },
                    )
                    RowDivider()
                    ChevronRow(
                        title = stringResource(R.string.remove_linux),
                        subtitle = stringResource(R.string.remove_linux_subtitle),
                        leading = { IconTile(Icons.Filled.Delete) },
                        modifier = Modifier.clickable(enabled = progress == null) { confirmRemove = true },
                    )
                }
                Spacer(Modifier.height(16.dp))
            }
            progress?.let { p ->
                val stage = when (p.stage) {
                    LinuxStage.DOWNLOADING -> stringResource(R.string.downloading)
                    LinuxStage.VERIFYING -> stringResource(R.string.verifying)
                    LinuxStage.UNPACKING -> stringResource(R.string.unpacking)
                    LinuxStage.CONFIGURING -> stringResource(R.string.configuring)
                }
                val name = linux.distros.firstOrNull { it.id == p.distroId }?.name ?: p.distroId
                SectionLabel(stringResource(R.string.installing_s, name))
                SectionCard {
                    Column(Modifier.padding(16.dp)) {
                        val detail = if (p.stage == LinuxStage.DOWNLOADING && p.total > 0u) {
                            "$stage · ${p.done.toLong() / 1_000_000} / ${p.total.toLong() / 1_000_000} MB"
                        } else {
                            stage
                        }
                        Text(detail, style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(10.dp))
                        if (p.stage == LinuxStage.DOWNLOADING && p.total > 0u) {
                            LinearProgressIndicator(progress = { p.done.toFloat() / p.total.toFloat() }, modifier = Modifier.fillMaxWidth())
                        } else {
                            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                        }
                    }
                }
                Spacer(Modifier.height(16.dp))
            }
            SectionLabel(stringResource(if (installed == null) R.string.choose_a_distribution else R.string.replace_with))
            SectionCard {
                linux.distros.forEachIndexed { i, d ->
                    if (i > 0) RowDivider()
                    // The installed image again is a no-op (same archive), so say so instead of offering it.
                    val current = installed?.distroId == d.id
                    ChevronRow(
                        title = "${d.name} ${d.version}",
                        subtitle = if (current) "${stringResource(R.string.installed)} · ${d.note}" else "${d.note} · ${d.downloadBytes.toLong() / 1_000_000} MB",
                        leading = { IconTile(if (current) Icons.Filled.Check else Icons.Filled.Download) },
                        modifier = Modifier.clickable(enabled = progress == null && !current) { linux.install(d.id) },
                    )
                }
            }
            Spacer(Modifier.height(16.dp))
            SectionLabel(stringResource(R.string.custom_rootfs))
            SectionCard {
                ChevronRow(
                    title = stringResource(R.string.custom_rootfs_from_file),
                    subtitle = stringResource(R.string.custom_rootfs_from_file_hint),
                    leading = { IconTile(Icons.Filled.FolderOpen) },
                    modifier = Modifier.clickable(enabled = progress == null) { pickFile.launch(arrayOf("*/*")) },
                )
                RowDivider()
                ChevronRow(
                    title = stringResource(R.string.custom_rootfs_from_url),
                    subtitle = stringResource(R.string.custom_rootfs_from_url_hint),
                    leading = { IconTile(Icons.Filled.Link) },
                    modifier = Modifier.clickable(enabled = progress == null) { urlDialog = true },
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }

    if (urlDialog) {
        var url by remember { mutableStateOf("") }
        var sha by remember { mutableStateOf("") }
        var name by remember { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { urlDialog = false },
            title = { Text(stringResource(R.string.custom_rootfs_from_url)) },
            text = {
                Column {
                    OutlinedTextField(url, { url = it }, label = { Text(stringResource(R.string.rootfs_url)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(sha, { sha = it }, label = { Text(stringResource(R.string.rootfs_sha256_optional)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(name, { name = it }, label = { Text(stringResource(R.string.rootfs_name_optional)) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                }
            },
            confirmButton = {
                TextButton(enabled = url.trim().startsWith("http"), onClick = {
                    urlDialog = false
                    linux.installFromUrl(url, sha, name)
                }) { Text(stringResource(R.string.install_action)) }
            },
            dismissButton = { TextButton(onClick = { urlDialog = false }) { Text(stringResource(android.R.string.cancel)) } },
        )
    }
}
