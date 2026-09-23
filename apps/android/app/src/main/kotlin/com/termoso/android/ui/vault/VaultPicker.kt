package com.termoso.android.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.android.ui.components.ChoiceSheet
import com.termoso.core.VaultInfo

/**
 * Top-bar title that names the current vault and, when there is more than
 * one, opens a bottom sheet to switch — like the vault selector in Termius.
 */
@Composable
fun VaultPickerTitle(
    vaults: List<VaultInfo>,
    selectedId: String?,
    onPick: (String) -> Unit,
) {
    val current = vaults.firstOrNull { it.id == selectedId }
    val switchable = vaults.size > 1
    var open by remember { mutableStateOf(false) }
    Row(
        Modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(enabled = switchable) { open = true }
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            current?.let { vaultLabel(it) } ?: stringResource(R.string.vault),
            style = MaterialTheme.typography.titleLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (switchable) {
            Icon(
                Icons.Filled.ExpandMore,
                contentDescription = stringResource(R.string.vaults),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    if (open) {
        ChoiceSheet(
            title = stringResource(R.string.vaults),
            options = vaults.map { v ->
                val label = vaultLabel(v)
                v.id to if (v.locked) "$label · ${stringResource(R.string.waiting_for_a_key)}" else label
            },
            selected = selectedId ?: "",
            onPick = onPick,
            onDismiss = { open = false },
        )
    }
}
