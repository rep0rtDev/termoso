package com.termoso.android.saf

import android.app.Application
import android.provider.DocumentsContract
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.FileNotFoundException

@RunWith(RobolectricTestRunner::class)
@Config(application = Application::class, sdk = [35])
class DocumentIdTest {
    private val host = "0f0e0d0c-0b0a-4908-8706-050403020100"

    @Test
    fun roundTrip() {
        val id = DocumentId(host, "/home/me/report (1).pdf")
        assertEquals(id, DocumentId.parse(id.encode()))
        assertEquals("$host:/home/me/report (1).pdf", id.encode())
        assertEquals("report (1).pdf", id.name)
        assertEquals(DocumentId(host, "/home/me"), id.parent)
    }

    @Test
    fun rootAndTopLevel() {
        val root = DocumentId.root(host)
        assertTrue(root.isRoot)
        assertEquals("$host:", root.encode())
        assertEquals(root, DocumentId.parse("$host:"))
        assertNull(root.parent)
        assertNull(DocumentId(host, "/").parent)
        assertEquals(DocumentId(host, "/"), DocumentId(host, "/etc").parent)
        assertEquals(DocumentId(host, "/etc"), DocumentId(host, "/").child("etc"))
        assertEquals(DocumentId(host, "/x"), root.child("x"))
    }

    @Test
    fun unicodeSpacesAndColonsSurvive() {
        val path = "/срв/Отчёты 2026/a:b:c.txt"
        val id = DocumentId.parse("$host:$path")
        assertEquals(DocumentId(host, path), id)
        assertEquals("a:b:c.txt", id!!.name)
    }

    @Test
    fun normalizesRedundantSegments() {
        assertEquals(DocumentId(host, "/a/b"), DocumentId.parse("$host:/a//./b/"))
        assertEquals(DocumentId(host, "/"), DocumentId.parse("$host:///"))
    }

    @Test
    fun rejectsTraversalAndMalformed() {
        assertNull(DocumentId.parse(null))
        assertNull(DocumentId.parse(""))
        assertNull(DocumentId.parse("nocolon"))
        assertNull(DocumentId.parse("$host:/a/../etc/passwd"))
        assertNull(DocumentId.parse("$host:/a/.."))
        assertNull(DocumentId.parse("$host:relative/path"))
        assertNull(DocumentId.parse("$host:/a\u0000b"))
        assertNull(DocumentId.parse("not-a-uuid:/etc"))
        assertNull(DocumentId.parse(host.uppercase() + ":/etc"))
        assertNull(DocumentId.parse("${host}extra:/etc"))
        assertNull(DocumentId.parse("locked:"))
        try {
            DocumentId.parseOrThrow("$host:/a/../b")
            error("expected failure")
        } catch (_: FileNotFoundException) {
        }
    }

    @Test
    fun childNamesAreValidated() {
        assertTrue(DocumentId.isValidName("notes.txt"))
        assertTrue(DocumentId.isValidName("Отчёт: итог"))
        assertFalse(DocumentId.isValidName(""))
        assertFalse(DocumentId.isValidName("."))
        assertFalse(DocumentId.isValidName(".."))
        assertFalse(DocumentId.isValidName("a/b"))
        assertFalse(DocumentId.isValidName("a\u0000b"))
        assertFalse(DocumentId.isValidName("x".repeat(256)))
        try {
            DocumentId(host, "/home").child("../etc")
            error("expected failure")
        } catch (_: IllegalArgumentException) {
        }
    }

    @Test
    fun containment() {
        val root = DocumentId.root(host)
        val home = DocumentId(host, "/home")
        assertTrue(root.contains(home))
        assertTrue(root.contains(DocumentId(host, "/")))
        assertTrue(home.contains(DocumentId(host, "/home/me/x")))
        assertFalse(home.contains(DocumentId(host, "/homework")))
        assertFalse(home.contains(home))
        assertTrue(DocumentId(host, "/").contains(home))
        assertFalse(DocumentId(host, "/").contains(DocumentId(host, "/")))
        assertFalse(home.contains(DocumentId("00000000-0000-4000-8000-000000000000", "/home/me")))
    }

    @Test
    fun mimeTypes() {
        val lookup: (String) -> String? = { ext -> mapOf("pdf" to "application/pdf", "txt" to "text/plain")[ext] }
        assertEquals(DocumentsContract.Document.MIME_TYPE_DIR, DocumentMime.of("anything.pdf", isDir = true, lookup))
        assertEquals("application/pdf", DocumentMime.of("Report.PDF", isDir = false, lookup))
        assertEquals("text/plain", DocumentMime.of("a.b.txt", isDir = false, lookup))
        assertEquals(DocumentMime.BINARY, DocumentMime.of("Makefile", isDir = false, lookup))
        assertEquals(DocumentMime.BINARY, DocumentMime.of(".bashrc", isDir = false, lookup))
        assertEquals(DocumentMime.BINARY, DocumentMime.of("blob.unknownext", isDir = false, lookup))
    }

    @Test
    fun uniqueNames() {
        assertEquals("a.txt", uniqueName("a.txt", emptySet()))
        assertEquals("a (1).txt", uniqueName("a.txt", setOf("a.txt")))
        assertEquals("a (2).txt", uniqueName("a.txt", setOf("a.txt", "a (1).txt")))
        assertEquals("dir (1)", uniqueName("dir", setOf("dir")))
        assertEquals(".env (1)", uniqueName(".env", setOf(".env")))
    }
}
