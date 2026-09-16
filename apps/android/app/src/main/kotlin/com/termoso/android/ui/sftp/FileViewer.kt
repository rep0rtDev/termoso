package com.termoso.android.ui.sftp

import android.graphics.Bitmap
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.OpenInNew
import androidx.compose.material.icons.filled.Save
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.termoso.android.ui.components.EmptyState

/**
 * Full-screen viewer/editor for one remote file. Text is shown read-only until the
 * pencil is tapped; Save writes the buffer back over the same SFTP session.
 * Back with unsaved edits asks first, so a stray gesture cannot lose work.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FileViewer(
    preview: PreviewState,
    snackbar: SnackbarHostState,
    onEdit: () -> Unit,
    onDraft: (String) -> Unit,
    onSave: () -> Unit,
    onDiscard: () -> Unit,
    onOpenWith: () -> Unit,
    onClose: () -> Unit,
) {
    var confirmClose by remember { mutableStateOf(false) }
    val close: () -> Unit = { if (preview.dirty) confirmClose = true else onClose() }
    BackHandler(onBack = close)

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(preview.entry.name, maxLines = 1)
                        Text(
                            when {
                                preview.saving -> "Saving…"
                                preview.dirty -> "Unsaved changes"
                                preview.editing -> "Editing"
                                else -> preview.entry.path
                            },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                        )
                    }
                },
                navigationIcon = {
                    IconButton(onClick = close) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") }
                },
                actions = {
                    when {
                        preview.editing -> {
                            IconButton(onClick = onDiscard, enabled = !preview.saving) {
                                Icon(Icons.Filled.Close, contentDescription = "Discard changes")
                            }
                            IconButton(onClick = onSave, enabled = preview.dirty && !preview.saving) {
                                Icon(Icons.Filled.Save, contentDescription = "Save")
                            }
                        }
                        preview.text != null -> IconButton(onClick = onEdit) {
                            Icon(Icons.Filled.Edit, contentDescription = "Edit")
                        }
                    }
                    IconButton(onClick = onOpenWith) { Icon(Icons.Filled.OpenInNew, contentDescription = "Open with") }
                },
            )
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding).imePadding()) {
            when {
                preview.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
                preview.error != null && preview.text == null && preview.image == null -> Box(
                    Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    EmptyState(title = "Could not open file", hint = preview.error, icon = Icons.Filled.Close)
                }
                preview.image != null -> ZoomableImage(preview.image)
                preview.draft != null -> TextEditor(preview.draft, onDraft, enabled = !preview.saving)
                preview.text != null -> TextView(preview.text)
            }
            if (preview.error != null && (preview.text != null || preview.image != null)) {
                Text(
                    preview.error,
                    color = MaterialTheme.colorScheme.onErrorContainer,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .fillMaxWidth()
                        .background(MaterialTheme.colorScheme.errorContainer)
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
        }
    }

    if (confirmClose) {
        AlertDialog(
            onDismissRequest = { confirmClose = false },
            title = { Text("Discard changes?") },
            text = { Text("${preview.entry.name} has unsaved edits.") },
            confirmButton = {
                Button(onClick = { confirmClose = false; onSave() }) { Text("Save") }
            },
            dismissButton = {
                TextButton(onClick = { confirmClose = false; onClose() }) { Text("Discard") }
            },
        )
    }
}

private val mono = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp, lineHeight = 18.sp)

@Composable
private fun TextView(text: String) {
    SelectionContainer {
        Text(
            text,
            style = mono,
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .horizontalScroll(rememberScrollState())
                .padding(12.dp),
        )
    }
}

@Composable
private fun TextEditor(draft: String, onDraft: (String) -> Unit, enabled: Boolean) {
    BasicTextField(
        value = draft,
        onValueChange = onDraft,
        enabled = enabled,
        textStyle = mono.copy(color = MaterialTheme.colorScheme.onSurface),
        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(12.dp),
    )
}

@Composable
private fun ZoomableImage(bitmap: Bitmap) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    Box(
        Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.surfaceContainerLowest)
            .pointerInput(Unit) {
                detectTransformGestures { _, pan, zoom, _ ->
                    scale = (scale * zoom).coerceIn(1f, 8f)
                    offset = if (scale == 1f) Offset.Zero else offset + pan
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Image(
            bitmap = bitmap.asImageBitmap(),
            contentDescription = null,
            modifier = Modifier
                .fillMaxSize()
                .graphicsLayer {
                    scaleX = scale
                    scaleY = scale
                    translationX = offset.x
                    translationY = offset.y
                },
        )
    }
}
