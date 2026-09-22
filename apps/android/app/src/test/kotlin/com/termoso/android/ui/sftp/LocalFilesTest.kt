package com.termoso.android.ui.sftp

import android.app.Application
import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [35])
class LocalFilesTest {
    private val context: Application = RuntimeEnvironment.getApplication()

    /** A picker-style provider: fixed rows per document id, bytes from a backing file. */
    class FakeDocuments : ContentProvider() {
        companion object {
            val rows = HashMap<String, Array<Any?>>()
            val files = HashMap<String, File>()
            const val AUTHORITY = "com.termoso.test.docs"
        }

        override fun onCreate() = true

        override fun query(uri: Uri, projection: Array<String>?, selection: String?, args: Array<String>?, sort: String?): Cursor {
            val cols = projection ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE, DocumentsContract.Document.COLUMN_LAST_MODIFIED)
            val row = rows[uri.lastPathSegment] ?: return MatrixCursor(cols)
            val (name, size, modified) = row
            val cursor = MatrixCursor(cols)
            cursor.addRow(
                cols.map {
                    when (it) {
                        OpenableColumns.DISPLAY_NAME -> name
                        OpenableColumns.SIZE -> size
                        DocumentsContract.Document.COLUMN_LAST_MODIFIED -> modified
                        else -> null
                    }
                },
            )
            return cursor
        }

        override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor =
            ParcelFileDescriptor.open(files.getValue(uri.lastPathSegment!!), ParcelFileDescriptor.MODE_READ_ONLY)

        override fun getType(uri: Uri): String? = null
        override fun insert(uri: Uri, values: ContentValues?): Uri? = null
        override fun delete(uri: Uri, selection: String?, args: Array<String>?) = 0
        override fun update(uri: Uri, values: ContentValues?, selection: String?, args: Array<String>?) = 0
    }

    private fun register(id: String, name: String, bytes: ByteArray, modified: Long?): Uri {
        val src = File.createTempFile("src", null, context.cacheDir).apply { writeBytes(bytes) }
        FakeDocuments.files[id] = src
        FakeDocuments.rows[id] = arrayOf(name, bytes.size.toLong(), modified)
        Robolectric.buildContentProvider(FakeDocuments::class.java).create(FakeDocuments.AUTHORITY)
        return Uri.parse("content://${FakeDocuments.AUTHORITY}/document/$id")
    }

    @Test
    fun copyKeepsDocumentModificationTime() {
        val modified = 1_600_000_000_000L
        val uri = register("photo", "photo.jpg", byteArrayOf(1, 2, 3), modified)
        val target = File(context.cacheDir, "ul/photo.jpg")

        LocalFiles.copyIn(context.contentResolver, uri, target)

        assertArrayEquals(byteArrayOf(1, 2, 3), target.readBytes())
        assertEquals(modified, target.lastModified())
        assertEquals(PickedDocument(uri, "photo.jpg", 3L), LocalFiles.describe(context.contentResolver, uri))
    }

    @Test
    fun providerWithoutModificationTimeLeavesCopyAsIs() {
        val uri = register("note", "note.txt", byteArrayOf(7), null)
        val target = File(context.cacheDir, "ul/note.txt")
        val before = System.currentTimeMillis() - 5_000

        LocalFiles.copyIn(context.contentResolver, uri, target)

        assertNull(LocalFiles.lastModified(context.contentResolver, uri))
        assert(target.lastModified() >= before) { "copy should keep its own fresh mtime" }
    }
}
