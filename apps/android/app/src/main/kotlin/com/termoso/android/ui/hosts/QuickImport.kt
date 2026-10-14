package com.termoso.android.ui.hosts

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.termoso.android.R

/** A host parsed out of pasted text or an `ssh_config` file. */
data class ImportedHost(val label: String, val address: String, val port: Int?, val username: String)

/**
 * Parses OpenSSH `~/.ssh/config` blocks and loose `user@host:port` / `ssh user@host -p N`
 * lines (Termius "Quick Import"). Wildcard `Host *` blocks are skipped.
 */
fun parseQuickImport(text: String): List<ImportedHost> {
    val out = LinkedHashMap<String, ImportedHost>()
    var cur: ImportedHost? = null
    fun flush() {
        cur?.takeIf { it.address.isNotBlank() && !it.address.contains('*') && !it.address.contains('?') }?.let { out[it.address + ":" + it.port + "@" + it.username] = it }
        cur = null
    }
    val loose = Regex("""^(?:ssh\s+)?(?:(?<user>[\w.\-]+)@)?(?<host>[\w.\-]+|\[[0-9a-fA-F:]+])(?::(?<port>\d{1,5}))?(?:\s+-p\s*(?<port2>\d{1,5}))?\s*$""")
    for (raw in text.lines()) {
        val line = raw.substringBefore('#').trim()
        if (line.isEmpty()) continue
        val kv = Regex("""^(\w+)\s*[=\s]\s*(.+)$""").find(line)
        val key = kv?.groupValues?.get(1)?.lowercase()
        when (key) {
            "host" -> {
                flush()
                val alias = kv.groupValues[2].trim().split(Regex("\\s+")).first()
                cur = ImportedHost(alias, alias, null, "")
            }
            "hostname" -> cur = cur?.copy(address = kv.groupValues[2].trim())
            "user" -> cur = cur?.copy(username = kv.groupValues[2].trim())
            "port" -> cur = cur?.copy(port = kv.groupValues[2].trim().toIntOrNull())
            "match" -> flush()
            else -> if (cur == null) {
                loose.find(line)?.let { m ->
                    val host = m.groups["host"]!!.value.trim('[', ']')
                    val port = (m.groups["port"] ?: m.groups["port2"])?.value?.toIntOrNull()
                    val h = ImportedHost(host, host, port, m.groups["user"]?.value.orEmpty())
                    out[h.address + ":" + h.port + "@" + h.username] = h
                }
            }
        }
    }
    flush()
    return out.values.toList()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun QuickImportSheet(onImport: (List<ImportedHost>) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    var text by remember { mutableStateOf("") }
    var scanning by remember { mutableStateOf(false) }
    if (scanning) {
        QrScanDialog(
            onResult = { code ->
                text = if (text.isBlank()) code else text + "\n" + code
                scanning = false
            },
            onDismiss = { scanning = false },
        )
    }
    val parsed = remember(text) { parseQuickImport(text) }
    val pick = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        runCatching { context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) } }
            .getOrNull()?.let { text = it }
    }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 24.dp)) {
            Text(stringResource(R.string.quick_import), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.quick_import_hint), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(12.dp))
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                modifier = Modifier.fillMaxWidth().heightIn(min = 120.dp, max = 240.dp),
                placeholder = { Text("Host web\n  HostName 203.0.113.5\n  User deploy\n\nroot@10.0.0.2:2222", fontFamily = FontFamily.Monospace) },
                textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            )
            Spacer(Modifier.height(12.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { pick.launch(arrayOf("*/*")) }) { Text(stringResource(R.string.choose_file)) }
                OutlinedButton(onClick = { scanning = true }) { Text(stringResource(R.string.scan_qr)) }
                Spacer(Modifier.weight(1f))
                Button(enabled = parsed.isNotEmpty(), onClick = { onImport(parsed) }) {
                    Text(pluralStringResource(R.plurals.import_n_hosts, parsed.size, parsed.size))
                }
            }
        }
    }
}
