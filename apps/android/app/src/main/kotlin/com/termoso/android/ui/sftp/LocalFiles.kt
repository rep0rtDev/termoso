package com.termoso.android.ui.sftp

import com.termoso.android.str
import com.termoso.android.R
import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import androidx.core.content.edit
import java.io.File

/** A document picked through SAF, with what we could learn about it without opening it. */
data class PickedDocument(val uri: Uri, val name: String, val size: Long?)

/** Everything SAF/Storage-related on the Kotlin side of a transfer; Rust only ever sees plain files. */
object LocalFiles {
    private const val PREFS = "sftp"
    private const val KEY_DOWNLOAD_TREE = "download_tree"

    fun describe(resolver: ContentResolver, uri: Uri): PickedDocument {
        var name: String? = null
        var size: Long? = null
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val n = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val s = c.getColumnIndex(OpenableColumns.SIZE)
                if (n >= 0) name = c.getString(n)
                if (s >= 0 && !c.isNull(s)) size = c.getLong(s)
            }
        }
        return PickedDocument(uri, name?.takeIf { it.isNotBlank() } ?: uri.lastPathSegment?.substringAfterLast('/') ?: "file", size)
    }

    /** Copy a picked document into [target] (parents created). */
    fun copyIn(resolver: ContentResolver, uri: Uri, target: File) {
        target.parentFile?.mkdirs()
        resolver.openInputStream(uri)?.use { input ->
            target.outputStream().use { output -> input.copyTo(output) }
        } ?: throw IllegalStateException(str(R.string.cannot_read_file, uri.lastPathSegment.orEmpty()))
    }

    /** One file or folder inside a picked SAF tree. */
    data class TreeChild(val uri: Uri, val name: String, val isDir: Boolean, val size: Long?)

    fun children(resolver: ContentResolver, tree: Uri, documentId: String): List<TreeChild> {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, documentId)
        val out = mutableListOf<TreeChild>()
        resolver.query(
            childrenUri,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_MIME_TYPE,
                DocumentsContract.Document.COLUMN_SIZE,
            ),
            null,
            null,
            null,
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getString(0)
                val name = c.getString(1) ?: continue
                val mime = c.getString(2)
                val size = if (c.isNull(3)) null else c.getLong(3)
                out += TreeChild(
                    uri = DocumentsContract.buildDocumentUriUsingTree(tree, id),
                    name = name,
                    isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR,
                    size = size,
                )
            }
        }
        return out
    }

    fun treeName(resolver: ContentResolver, tree: Uri): String {
        val doc = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        return describe(resolver, doc).name
    }

    fun downloadTree(context: Context): Uri? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_DOWNLOAD_TREE, null)?.let(Uri::parse)
            ?.takeIf { uri -> context.contentResolver.persistedUriPermissions.any { it.uri == uri && it.isWritePermission } }

    /** Remember the folder the user picked for downloads, keeping its permission across restarts. */
    fun setDownloadTree(context: Context, tree: Uri) {
        context.contentResolver.takePersistableUriPermission(
            tree,
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
        )
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit { putString(KEY_DOWNLOAD_TREE, tree.toString()) }
    }

    /** Human label of the chosen download folder (`Downloads/servers`), or null when none is set. */
    fun downloadTreeLabel(context: Context): String? {
        val tree = downloadTree(context) ?: return null
        val id = runCatching { DocumentsContract.getTreeDocumentId(tree) }.getOrNull() ?: return null
        return id.substringAfter(':', id).ifBlank { runCatching { treeName(context.contentResolver, tree) }.getOrDefault("folder") }
    }

    /**
     * Copy a finished download into the chosen download folder; an existing
     * document with the same name is replaced (SAF would otherwise add ` (1)`).
     * Returns the created document.
     */
    fun saveToTree(context: Context, tree: Uri, file: File): Uri {
        val resolver = context.contentResolver
        val parentId = DocumentsContract.getTreeDocumentId(tree)
        val parent = DocumentsContract.buildDocumentUriUsingTree(tree, parentId)
        children(resolver, tree, parentId).firstOrNull { it.name == file.name && !it.isDir }?.let {
            runCatching { DocumentsContract.deleteDocument(resolver, it.uri) }
        }
        val doc = DocumentsContract.createDocument(resolver, parent, mimeOf(file.name), file.name)
            ?: throw IllegalStateException(str(R.string.could_not_create_in_the_download_folder, file.name))
        resolver.openOutputStream(doc, "wt")?.use { output ->
            file.inputStream().use { input -> input.copyTo(output) }
        } ?: throw IllegalStateException(str(R.string.could_not_write, file.name))
        return doc
    }

    fun mimeOf(name: String): String {
        val ext = name.substringAfterLast('.', "").lowercase()
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: if (ext.isEmpty()) "application/octet-stream" else "*/*"
    }

    /** Android "Open with" for a file in our cache, shared read-only through the FileProvider. */
    fun openWithIntent(context: Context, file: File): Intent {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
        return Intent(Intent.ACTION_VIEW)
            .setDataAndType(uri, mimeOf(file.name))
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
}
