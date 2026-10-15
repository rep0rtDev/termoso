package com.termoso.android.ui.terminal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.termoso.android.R

/**
 * A roomy input box for the terminal: longer prompts for CLI coding agents,
 * multi-line commands, dictation. Text is only sent when the user presses
 * Send (paste + Enter) or Paste (no Enter) — nothing goes out while typing.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ComposeSheet(
    text: String,
    onTextChange: (String) -> Unit,
    onVoice: () -> Unit,
    onPaste: (String) -> Unit,
    onSend: (String) -> Unit,
    onClose: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onClose) {
        Column(Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, bottom = 24.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(stringResource(R.string.compose_message), style = MaterialTheme.typography.titleMedium)
            Text(
                stringResource(R.string.compose_hint),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedTextField(
                value = text,
                onValueChange = onTextChange,
                minLines = 3,
                maxLines = 8,
                modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                trailingIcon = {
                    IconButton(onClick = onVoice) {
                        Icon(Icons.Filled.Mic, contentDescription = stringResource(R.string.voice_input))
                    }
                },
            )
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Spacer(Modifier.weight(1f))
                TextButton(onClick = { onPaste(text) }, enabled = text.isNotEmpty()) { Text(stringResource(R.string.paste_without_enter)) }
                Button(onClick = { onSend(text) }, enabled = text.isNotBlank()) { Text(stringResource(R.string.send)) }
            }
        }
    }
}
