package com.termoso.android.ui.connections

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbManager
import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Usb
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.data.serial.UsbSerial
import com.termoso.core.SerialLine
import com.termoso.core.defaultSerialLine
import com.termoso.core.serialBaudRates

/**
 * Pick an attached USB-UART adapter and its line settings (115200 8N1 by
 * default), then open it as a terminal. The list follows plug/unplug events.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SerialSheet(
    onConnect: (adapterKey: String, line: SerialLine) -> Unit,
    onDismiss: () -> Unit,
) {
    val context = LocalContext.current
    var adapters by remember { mutableStateOf(UsbSerial.adapters(context)) }
    var selected by remember { mutableStateOf(adapters.firstOrNull()?.key) }
    var line by remember { mutableStateOf(defaultSerialLine()) }

    DisposableEffect(Unit) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(ctx: Context, intent: Intent) {
                adapters = UsbSerial.adapters(ctx)
                if (adapters.none { it.key == selected }) selected = adapters.firstOrNull()?.key
            }
        }
        val filter = IntentFilter().apply {
            addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
            addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            context.registerReceiver(receiver, filter)
        }
        onDispose { runCatching { context.unregisterReceiver(receiver) } }
    }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 20.dp)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Icon(Icons.Filled.Usb, contentDescription = null)
                Text(stringResource(R.string.serial_usb), style = MaterialTheme.typography.titleLarge)
            }
            if (adapters.isEmpty()) {
                Text(stringResource(R.string.no_usb_adapters), style = MaterialTheme.typography.bodyLarge)
                Text(
                    stringResource(R.string.plug_in_usb_adapter),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
                    adapters.forEach { a ->
                        FilterChip(
                            selected = a.key == selected,
                            onClick = { selected = a.key },
                            label = { Text(a.name) },
                        )
                    }
                }
            }

            Picker(
                label = stringResource(R.string.baud_rate),
                value = line.baudRate.toString(),
                options = serialBaudRates().map { it.toString() to it.toString() },
            ) { line = line.copy(baudRate = it.toUInt()) }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Picker(
                    label = stringResource(R.string.data_bits),
                    value = line.dataBits.toString(),
                    options = listOf("8", "7", "6", "5").map { it to it },
                    modifier = Modifier.weight(1f),
                ) { line = line.copy(dataBits = it.toUByte()) }
                Picker(
                    label = stringResource(R.string.stop_bits),
                    value = line.stopBits.toString(),
                    options = listOf("1", "2").map { it to it },
                    modifier = Modifier.weight(1f),
                ) { line = line.copy(stopBits = it.toUByte()) }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Picker(
                    label = stringResource(R.string.parity),
                    value = line.parity,
                    options = listOf(
                        "none" to stringResource(R.string.parity_none),
                        "odd" to stringResource(R.string.parity_odd),
                        "even" to stringResource(R.string.parity_even),
                        "mark" to stringResource(R.string.parity_mark),
                        "space" to stringResource(R.string.parity_space),
                    ),
                    modifier = Modifier.weight(1f),
                ) { line = line.copy(parity = it) }
                Picker(
                    label = stringResource(R.string.flow_control),
                    value = line.flowControl,
                    options = listOf(
                        "none" to stringResource(R.string.flow_none),
                        "hardware" to stringResource(R.string.flow_hardware),
                        "software" to stringResource(R.string.flow_software),
                    ),
                    modifier = Modifier.weight(1f),
                ) { line = line.copy(flowControl = it) }
            }
            Picker(
                label = stringResource(R.string.charset),
                value = line.charset.ifEmpty { "utf-8" },
                options = CHARSETS.map { it to it.uppercase() },
            ) { line = line.copy(charset = if (it == "utf-8") "" else it) }

            Button(
                onClick = { selected?.let { onConnect(it, line) } },
                enabled = selected != null,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.open_console))
            }
        }
    }
}

private val CHARSETS = listOf(
    "utf-8", "iso-8859-1", "iso-8859-2", "iso-8859-15", "windows-1250", "windows-1251",
    "windows-1252", "koi8-r", "koi8-u", "gbk", "gb18030", "big5", "shift_jis", "euc-jp", "euc-kr",
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Picker(
    label: String,
    value: String,
    options: List<Pair<String, String>>,
    modifier: Modifier = Modifier,
    onPick: (String) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }, modifier = modifier) {
        OutlinedTextField(
            value = options.firstOrNull { it.first == value }?.second ?: value,
            onValueChange = {},
            readOnly = true,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = open) },
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(MenuAnchorType.PrimaryNotEditable),
            singleLine = true,
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEach { (key, text) ->
                DropdownMenuItem(text = { Text(text) }, onClick = { onPick(key); open = false })
            }
        }
    }
}
