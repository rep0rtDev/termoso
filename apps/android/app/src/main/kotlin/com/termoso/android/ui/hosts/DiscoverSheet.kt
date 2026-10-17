package com.termoso.android.ui.hosts

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Dns
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import kotlinx.coroutines.delay

/** An SSH server advertised over mDNS/Bonjour on the local network. */
data class DiscoveredHost(val name: String, val address: String, val port: Int) {
    val target: String get() = if (port == 22) address else "$address:$port"
}

private val SERVICE_TYPES = listOf("_ssh._tcp.", "_sftp-ssh._tcp.")

/** How long the spinner runs; browsing keeps going quietly afterwards until Rescan or dismiss. */
private const val SCAN_WINDOW_MS = 12_000L

/**
 * Browses the local network for `_ssh._tcp` / `_sftp-ssh._tcp` services (Termius "Discover")
 * and offers each one as a new-host target. Nothing is sent anywhere but multicast DNS.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiscoverSheet(onPick: (String) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    val found = remember { mutableStateListOf<DiscoveredHost>() }
    var error by remember { mutableStateOf<String?>(null) }
    var generation by remember { mutableIntStateOf(0) }
    var scanning by remember { mutableStateOf(true) }

    LaunchedEffect(generation) {
        scanning = true
        delay(SCAN_WINDOW_MS)
        scanning = false
    }
    DisposableEffect(generation) {
        val nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
        val listeners = SERVICE_TYPES.map { type ->
            object : NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(serviceType: String) {}
                override fun onDiscoveryStopped(serviceType: String) {}
                override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) { error = "NSD error $errorCode" }
                override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
                override fun onServiceLost(serviceInfo: NsdServiceInfo) {
                    found.removeAll { it.name == serviceInfo.serviceName }
                }
                override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                    @Suppress("DEPRECATION")
                    nsd.resolveService(serviceInfo, object : NsdManager.ResolveListener {
                        override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {}
                        override fun onServiceResolved(info: NsdServiceInfo) {
                            @Suppress("DEPRECATION")
                            val addr = info.host?.hostAddress ?: return
                            val host = DiscoveredHost(info.serviceName, addr, info.port)
                            if (found.none { it.address == host.address && it.port == host.port }) found.add(host)
                        }
                    })
                }
            }.also { nsd.discoverServices(type, NsdManager.PROTOCOL_DNS_SD, it) }
        }
        onDispose { listeners.forEach { runCatching { nsd.stopServiceDiscovery(it) } } }
    }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.padding(bottom = 24.dp)) {
            Row(Modifier.padding(horizontal = 20.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(stringResource(R.string.discover_on_local_network), style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.weight(1f))
                when {
                    error != null -> {}
                    scanning -> CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    else -> TextButton(onClick = { generation++ }) { Text(stringResource(R.string.rescan)) }
                }
            }
            Spacer(Modifier.height(8.dp))
            val hint = error ?: when {
                found.isNotEmpty() -> null
                scanning -> stringResource(R.string.looking_for_ssh_servers)
                else -> stringResource(R.string.no_ssh_servers_found)
            }
            if (hint != null) {
                Text(hint, Modifier.padding(horizontal = 20.dp, vertical = 8.dp), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            LazyColumn(Modifier.fillMaxWidth()) {
                items(found, key = { it.address + ":" + it.port }) { h ->
                    ListItem(
                        headlineContent = { Text(h.name) },
                        supportingContent = { Text(h.target) },
                        leadingContent = { Icon(Icons.Filled.Dns, contentDescription = null) },
                        modifier = Modifier.clickable { onPick(h.target) },
                    )
                }
            }
        }
    }
}
