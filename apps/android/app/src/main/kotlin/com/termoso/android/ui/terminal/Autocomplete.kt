package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Subject
import androidx.compose.material.icons.filled.DataObject
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.InsertDriveFile
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.termoso.core.SessionState
import com.termoso.core.SuggestionItem
import com.termoso.core.SuggestionKind

/** Quiet time after the last rendered frame before the typed line is re-read. */
const val AUTOCOMPLETE_DEBOUNCE_MS = 90L

/** Longest chip text; longer history lines are cut in the middle. */
const val SUGGESTION_MAX_CHARS = 40

/**
 * Whether suggestions may be shown at all: the setting is on, the shell is
 * up and this terminal accepts our input (not a read-only view of somebody
 * else's session).
 */
fun autocompleteAllowed(enabled: Boolean, state: SessionState, canWrite: Boolean, isView: Boolean): Boolean =
    enabled && !isView && canWrite && state is SessionState.Connected

/**
 * What to insert for a tapped suggestion: [SuggestionItem.insert] verbatim,
 * except that anything containing a line break is refused so a suggestion
 * can never execute (`null` → ignore the tap).
 */
fun suggestionInsert(item: SuggestionItem): String? =
    item.insert.takeUnless { it.contains('\n') || it.contains('\r') }

/** Chip text: the whole label when short, otherwise `start…end`. */
fun suggestionChipText(item: SuggestionItem, max: Int = SUGGESTION_MAX_CHARS): String {
    val label = item.label
    if (label.length <= max) return label
    val head = (max - 1) * 2 / 3
    val tail = max - 1 - head
    return label.take(head) + "…" + label.takeLast(tail)
}

/** Secondary text on the chip, or `null` when it would only repeat the label. */
fun suggestionChipDetail(item: SuggestionItem): String? = when (item.kind) {
    SuggestionKind.HISTORY -> null
    SuggestionKind.PATH -> null
    else -> item.desc.takeIf { it.isNotBlank() && it != item.label }
}

/** Icon standing for where a suggestion came from. */
fun suggestionIcon(item: SuggestionItem): ImageVector = when (item.kind) {
    SuggestionKind.COMMAND -> Icons.Filled.Terminal
    SuggestionKind.OPTION -> Icons.Filled.Tune
    SuggestionKind.SUBCOMMAND -> Icons.AutoMirrored.Filled.Subject
    SuggestionKind.PATH -> if (item.label.endsWith("/")) Icons.Filled.Folder else Icons.Filled.InsertDriveFile
    SuggestionKind.HISTORY -> Icons.Filled.History
    SuggestionKind.SNIPPET -> Icons.Filled.DataObject
}

/**
 * Remembers what the strip last showed so a frame that did not change the
 * typed line costs nothing. [next] returns the items to show, or `null`
 * when the caller should keep what it has.
 */
class AutocompleteTracker {
    private var line: String? = null

    /**
     * [typed] is what Rust reports for the current prompt (`null` = nothing
     * to complete). [fetch] is only invoked when the line changed to
     * something non-blank.
     */
    fun next(typed: String?, fetch: () -> List<SuggestionItem>): List<SuggestionItem>? {
        if (typed == line) return null
        line = typed
        return if (typed.isNullOrBlank()) emptyList() else fetch()
    }

    /** Forget the last line so the next frame refreshes unconditionally. */
    fun reset() {
        line = null
    }
}

/** One row of tappable suggestion chips; renders nothing for an empty list. */
@Composable
fun AutocompleteStrip(
    items: List<SuggestionItem>,
    onPick: (SuggestionItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (items.isEmpty()) return
    LazyRow(
        modifier = modifier
            .fillMaxWidth()
            .height(36.dp)
            .background(MaterialTheme.colorScheme.surfaceContainer)
            .testTag("autocomplete"),
        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        items(items, key = { it.kind.name + it.label }) { item ->
            SuggestionChip(item, onClick = { onPick(item) })
        }
    }
}

@Composable
private fun SuggestionChip(item: SuggestionItem, onClick: () -> Unit) {
    val detail = suggestionChipDetail(item)
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Icon(
            suggestionIcon(item),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(14.dp),
        )
        Text(
            suggestionChipText(item),
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = MaterialTheme.colorScheme.onSurface,
        )
        if (detail != null) {
            Text(
                detail,
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.widthIn(max = 160.dp),
            )
        }
    }
}
