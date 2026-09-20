package com.termoso.android.saf

import android.content.res.AssetFileDescriptor
import android.database.Cursor
import android.database.MatrixCursor
import android.graphics.Point
import android.os.Bundle
import android.os.CancellationSignal
import android.os.Handler
import android.os.HandlerThread
import android.os.ParcelFileDescriptor
import android.os.ProxyFileDescriptorCallback
import android.os.storage.StorageManager
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import android.system.ErrnoException
import android.system.OsConstants
import android.util.Log
import com.termoso.android.R
import com.termoso.android.TermosoApplication
import com.termoso.android.data.AppContainer
import com.termoso.android.data.SftpConnection
import com.termoso.android.data.userMessage
import com.termoso.android.str
import com.termoso.core.FileMode
import com.termoso.core.MobileException
import com.termoso.core.SftpEntry
import com.termoso.core.SftpFile
import java.io.FileNotFoundException
import java.util.concurrent.ConcurrentHashMap

/**
 * Saved SSH hosts as storage roots for the system Files UI and any app that
 * uses the storage access framework. Directories list, files open for reading
 * and writing through a seekable proxy descriptor (ranges go straight to the
 * remote handle; nothing is buffered whole), and create / rename / move /
 * delete map onto the matching SFTP calls.
 *
 * What other apps get is the remote filesystem the host account already sees,
 * nothing from the vault: no credentials, no keys, no host records beyond a
 * label and address on the root. Ids are validated on every call
 * ([DocumentId]); a locked vault is a single explanatory root, never a prompt.
 */
class SftpDocumentsProvider : DocumentsProvider() {
    // Providers are attached before Application.onCreate(), so the container
    // is resolved on first use rather than here.
    private val container: AppContainer by lazy {
        (context?.applicationContext as TermosoApplication).container
    }
    private val access: SftpAccess by lazy { SftpAccess(container) }
    private val homes = ConcurrentHashMap<String, String>()
    private var ioThreadStarted = false
    private val ioThread by lazy { HandlerThread("termoso-saf").apply { start(); ioThreadStarted = true } }
    private val ioHandler by lazy { Handler(ioThread.looper) }

    override fun onCreate(): Boolean = context?.applicationContext is TermosoApplication

    override fun shutdown() {
        if (ioThreadStarted) ioThread.quitSafely()
        super.shutdown()
    }

    // ---- roots ---------------------------------------------------------

    override fun queryRoots(projection: Array<String>?): Cursor {
        val cursor = MatrixCursor(projection ?: ROOT_COLUMNS)
        if (!container.files.enabled.value) return cursor
        if (!access.available()) {
            if (container.hasProfile()) lockedRoot(cursor)
            return cursor
        }
        val vault = try {
            access.vault()
        } catch (e: ProviderException) {
            lockedRoot(cursor)
            return cursor
        }
        val hosts = try {
            access.hosts(vault)
        } catch (e: ProviderException) {
            return cursor
        }
        for (host in hosts) {
            val user = host.username.takeIf { it.isNotBlank() }?.let { "$it@" } ?: ""
            cursor.newRow()
                .add(Root.COLUMN_ROOT_ID, host.id)
                .add(Root.COLUMN_DOCUMENT_ID, DocumentId.root(host.id).encode())
                .add(Root.COLUMN_TITLE, host.label.ifBlank { host.address })
                .add(Root.COLUMN_SUMMARY, "$user${host.address}")
                .add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE or Root.FLAG_SUPPORTS_IS_CHILD)
                .add(Root.COLUMN_ICON, R.mipmap.ic_launcher)
                .add(Root.COLUMN_MIME_TYPES, "*/*")
        }
        return cursor
    }

    private fun lockedRoot(cursor: MatrixCursor) {
        cursor.newRow()
            .add(Root.COLUMN_ROOT_ID, RootIds.LOCKED)
            .add(Root.COLUMN_DOCUMENT_ID, RootIds.LOCKED_DOCUMENT)
            .add(Root.COLUMN_TITLE, str(R.string.app_name))
            .add(Root.COLUMN_SUMMARY, str(R.string.files_locked_root_summary))
            .add(Root.COLUMN_FLAGS, 0)
            .add(Root.COLUMN_ICON, R.mipmap.ic_launcher)
            .add(Root.COLUMN_MIME_TYPES, Document.MIME_TYPE_DIR)
    }

    // ---- documents -----------------------------------------------------

    override fun queryDocument(documentId: String, projection: Array<String>?): Cursor {
        val cursor = MatrixCursor(projection ?: DOCUMENT_COLUMNS)
        if (documentId == RootIds.LOCKED_DOCUMENT) {
            cursor.newRow()
                .add(Document.COLUMN_DOCUMENT_ID, documentId)
                .add(Document.COLUMN_DISPLAY_NAME, str(R.string.app_name))
                .add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR)
                .add(Document.COLUMN_FLAGS, 0)
            return cursor
        }
        val id = DocumentId.parseOrThrow(documentId)
        val vault = access.vault()
        if (id.isRoot) {
            val host = access.host(vault, id.hostId)
            cursor.newRow()
                .add(Document.COLUMN_DOCUMENT_ID, documentId)
                .add(Document.COLUMN_DISPLAY_NAME, host.label.ifBlank { host.address })
                .add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR)
                .add(Document.COLUMN_FLAGS, Document.FLAG_DIR_SUPPORTS_CREATE)
            return cursor
        }
        val conn = access.connection(id.hostId)
        val entry = sftp { conn.rust.stat(id.path) }
        addEntry(cursor, id, entry)
        return cursor
    }

    override fun queryChildDocuments(parentDocumentId: String, projection: Array<String>?, sortOrder: String?): Cursor {
        val cursor = MatrixCursor(projection ?: DOCUMENT_COLUMNS)
        val ctx = context ?: return cursor
        cursor.setNotificationUri(ctx.contentResolver, DocumentsContract.buildChildDocumentsUri(FilesIntegration.AUTHORITY, parentDocumentId))
        if (parentDocumentId == RootIds.LOCKED_DOCUMENT) {
            return cursor.withError(str(R.string.files_unlock_termoso_first))
        }
        val parent = DocumentId.parseOrThrow(parentDocumentId)
        return try {
            val conn = access.connection(parent.hostId)
            val dir = resolveDir(conn, parent)
            for (entry in sftp { conn.rust.list(dir) }) {
                if (!DocumentId.isValidName(entry.name)) continue
                addEntry(cursor, DocumentId(parent.hostId, DocumentId.joinPath(dir, entry.name)), entry)
            }
            cursor
        } catch (e: ProviderException) {
            cursor.withError(e.message ?: e.toString())
        }
    }

    override fun isChildDocument(parentDocumentId: String, documentId: String): Boolean {
        val parent = DocumentId.parse(parentDocumentId) ?: return false
        val child = DocumentId.parse(documentId) ?: return false
        return parent.contains(child)
    }

    // ---- content -------------------------------------------------------

    override fun openDocument(documentId: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        val id = DocumentId.parseOrThrow(documentId)
        if (id.isRoot) throw FileNotFoundException("Not a file")
        val ctx = context ?: throw FileNotFoundException("No context")
        val pfdMode = ParcelFileDescriptor.parseMode(mode)
        val write = pfdMode and (ParcelFileDescriptor.MODE_WRITE_ONLY or ParcelFileDescriptor.MODE_READ_WRITE) != 0
        val truncate = pfdMode and ParcelFileDescriptor.MODE_TRUNCATE != 0
        val conn = access.connection(id.hostId)
        val file = sftp {
            when {
                !write -> conn.rust.openFile(id.path, FileMode.READ)
                truncate && pfdMode and ParcelFileDescriptor.MODE_READ_WRITE == 0 -> conn.rust.openFile(id.path, FileMode.WRITE)
                else -> conn.rust.openFile(id.path, FileMode.READ_WRITE).also { if (truncate) it.truncate(0uL) }
            }
        }
        val storage = ctx.getSystemService(StorageManager::class.java)
        val parents = parentsOf(conn, id)
        val callback = RemoteFileCallback(file) {
            if (write) parents.forEach(::changed)
        }
        return try {
            storage.openProxyFileDescriptor(pfdMode, callback, ioHandler)
        } catch (e: Exception) {
            file.use { runCatching { it.release() } }
            throw FileNotFoundException(e.message ?: e.toString())
        }
    }

    override fun openDocumentThumbnail(documentId: String, sizeHint: Point, signal: CancellationSignal?): AssetFileDescriptor {
        throw FileNotFoundException("No thumbnails")
    }

    /** Range reads and writes against one remote handle, on the provider's own thread. */
    private class RemoteFileCallback(private val file: SftpFile, private val onReleased: () -> Unit) : ProxyFileDescriptorCallback() {
        override fun onGetSize(): Long = errno { file.size().toLong() }

        override fun onRead(offset: Long, size: Int, data: ByteArray): Int = errno {
            val chunk = file.readAt(offset.toULong(), size.toUInt())
            chunk.copyInto(data, 0, 0, chunk.size)
            chunk.size
        }

        override fun onWrite(offset: Long, size: Int, data: ByteArray): Int = errno {
            file.writeAt(offset.toULong(), if (size == data.size) data else data.copyOf(size))
            size
        }

        override fun onFsync() {
            errno { file.sync() }
        }

        override fun onRelease() {
            file.use { f -> runCatching { f.release() }.onFailure { Log.w(TAG, "release: ${it.message}") } }
            onReleased()
        }

        private inline fun <T> errno(block: () -> T): T = try {
            block()
        } catch (e: MobileException.NotFound) {
            throw ErrnoException("sftp", OsConstants.ENOENT)
        } catch (e: MobileException.Closed) {
            throw ErrnoException("sftp", OsConstants.EIO)
        } catch (e: MobileException) {
            Log.w(TAG, "sftp: ${e.message}")
            throw ErrnoException("sftp", OsConstants.EIO)
        }
    }

    // ---- mutations -----------------------------------------------------

    override fun createDocument(parentDocumentId: String, mimeType: String, displayName: String): String {
        val parent = DocumentId.parseOrThrow(parentDocumentId)
        if (!DocumentId.isValidName(displayName)) throw FileNotFoundException(str(R.string.files_invalid_name))
        val conn = access.connection(parent.hostId)
        val dir = resolveDir(conn, parent)
        val taken = sftp { conn.rust.list(dir) }.map { it.name }.toSet()
        val name = uniqueName(displayName, taken)
        val target = DocumentId(parent.hostId, DocumentId.joinPath(dir, name))
        sftp {
            if (mimeType == Document.MIME_TYPE_DIR) {
                conn.rust.mkdir(target.path)
            } else {
                conn.rust.openFile(target.path, FileMode.WRITE).use { it.release() }
            }
        }
        changed(parentDocumentId)
        return target.encode()
    }

    override fun deleteDocument(documentId: String) {
        val id = DocumentId.parseOrThrow(documentId)
        if (id.isRoot || id.path == "/") throw FileNotFoundException(str(R.string.files_cannot_remove_root))
        val conn = access.connection(id.hostId)
        sftp { conn.rust.remove(id.path) }
        revokeDocumentPermission(documentId)
        parentsOf(conn, id).forEach(::changed)
    }

    override fun renameDocument(documentId: String, displayName: String): String? {
        val id = DocumentId.parseOrThrow(documentId)
        if (id.isRoot || id.path == "/") throw FileNotFoundException(str(R.string.files_cannot_remove_root))
        if (!DocumentId.isValidName(displayName)) throw FileNotFoundException(str(R.string.files_invalid_name))
        if (displayName == id.name) return null
        val parent = id.parent ?: throw FileNotFoundException(str(R.string.files_cannot_remove_root))
        val target = parent.child(displayName)
        val conn = access.connection(id.hostId)
        sftp {
            if (conn.rust.exists(target.path)) throw ProviderException(str(R.string.files_name_taken, displayName))
            conn.rust.rename(id.path, target.path)
        }
        revokeDocumentPermission(documentId)
        parentsOf(conn, id).forEach(::changed)
        return target.encode()
    }

    override fun moveDocument(sourceDocumentId: String, sourceParentDocumentId: String, targetParentDocumentId: String): String {
        val source = DocumentId.parseOrThrow(sourceDocumentId)
        val sourceParent = DocumentId.parseOrThrow(sourceParentDocumentId)
        val targetParent = DocumentId.parseOrThrow(targetParentDocumentId)
        if (source.isRoot || source.path == "/") throw FileNotFoundException(str(R.string.files_cannot_remove_root))
        if (!sourceParent.contains(source)) throw FileNotFoundException(str(R.string.files_host_no_longer_exists))
        if (source.hostId != targetParent.hostId) throw UnsupportedOperationException(str(R.string.files_move_between_hosts))
        val conn = access.connection(source.hostId)
        val dir = resolveDir(conn, targetParent)
        val target = DocumentId(source.hostId, DocumentId.joinPath(dir, source.name))
        if (target == source) return sourceDocumentId
        if (source.contains(target)) throw FileNotFoundException(str(R.string.files_move_into_itself))
        sftp {
            if (conn.rust.exists(target.path)) throw ProviderException(str(R.string.files_name_taken, source.name))
            conn.rust.rename(source.path, target.path)
        }
        revokeDocumentPermission(sourceDocumentId)
        changed(sourceParentDocumentId)
        changed(targetParentDocumentId)
        return target.encode()
    }

    // ---- helpers -------------------------------------------------------

    /** The remote directory behind [id]: the home directory for a host root. */
    private fun resolveDir(conn: SftpConnection, id: DocumentId): String =
        if (id.isRoot) home(conn) else id.path

    private fun home(conn: SftpConnection): String =
        homes[conn.id] ?: sftp { conn.rust.canonicalize("~") }.also { homes[conn.id] = it }

    /**
     * Ids whose child listings show [id]: its directory, plus the host root
     * when that directory is the home directory (the root lists it under a
     * different id).
     */
    private fun parentsOf(conn: SftpConnection, id: DocumentId): List<String> {
        val parent = id.parent ?: return emptyList()
        val ids = mutableListOf(parent.encode())
        if (parent.path == runCatching { home(conn) }.getOrNull()) ids += DocumentId.root(id.hostId).encode()
        return ids
    }

    private fun addEntry(cursor: MatrixCursor, id: DocumentId, entry: SftpEntry) {
        var flags = Document.FLAG_SUPPORTS_DELETE or Document.FLAG_SUPPORTS_RENAME or Document.FLAG_SUPPORTS_MOVE
        flags = flags or if (entry.isDir) Document.FLAG_DIR_SUPPORTS_CREATE else Document.FLAG_SUPPORTS_WRITE
        val row = cursor.newRow()
            .add(Document.COLUMN_DOCUMENT_ID, id.encode())
            .add(Document.COLUMN_DISPLAY_NAME, entry.name.ifEmpty { id.name })
            .add(Document.COLUMN_MIME_TYPE, DocumentMime.of(entry.name, entry.isDir))
            .add(Document.COLUMN_FLAGS, flags)
            .add(Document.COLUMN_SUMMARY, listOfNotNull(entry.permissions, entry.owner).joinToString("  "))
        entry.size?.let { if (!entry.isDir) row.add(Document.COLUMN_SIZE, it.toLong()) }
        entry.modifiedMs?.let { row.add(Document.COLUMN_LAST_MODIFIED, it) }
    }

    private fun changed(documentId: String) {
        context?.contentResolver?.notifyChange(DocumentsContract.buildChildDocumentsUri(FilesIntegration.AUTHORITY, documentId), null)
    }

    private fun MatrixCursor.withError(message: String): MatrixCursor {
        extras = Bundle().apply { putString(DocumentsContract.EXTRA_ERROR, message) }
        return this
    }

    /** Run one blocking SFTP call, turning core failures into what SAF callers show. */
    private inline fun <T> sftp(block: () -> T): T = try {
        block()
    } catch (e: MobileException.NotFound) {
        throw FileNotFoundException(e.userMessage())
    } catch (e: MobileException) {
        throw ProviderException(e.userMessage())
    }

    private companion object {
        const val TAG = "SftpDocuments"

        val ROOT_COLUMNS = arrayOf(
            Root.COLUMN_ROOT_ID,
            Root.COLUMN_DOCUMENT_ID,
            Root.COLUMN_TITLE,
            Root.COLUMN_SUMMARY,
            Root.COLUMN_FLAGS,
            Root.COLUMN_ICON,
            Root.COLUMN_MIME_TYPES,
        )

        val DOCUMENT_COLUMNS = arrayOf(
            Document.COLUMN_DOCUMENT_ID,
            Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE,
            Document.COLUMN_FLAGS,
            Document.COLUMN_SIZE,
            Document.COLUMN_LAST_MODIFIED,
            Document.COLUMN_SUMMARY,
        )
    }
}
