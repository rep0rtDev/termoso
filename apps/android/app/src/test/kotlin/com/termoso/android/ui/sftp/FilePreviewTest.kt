package com.termoso.android.ui.sftp

import com.termoso.android.ResourceTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FilePreviewTest : ResourceTest() {
    @Test
    fun extensionsPickTheViewer() {
        assertEquals(PreviewKind.Text, FilePreview.classify("nginx.conf", 1024))
        assertEquals(PreviewKind.Text, FilePreview.classify("Main.kt", null))
        assertEquals(PreviewKind.Image, FilePreview.classify("photo.JPG", 2_000_000))
        assertEquals(PreviewKind.Image, FilePreview.classify("icon.webp", null))
    }

    @Test
    fun wellKnownExtensionlessFilesAreText() {
        assertEquals(PreviewKind.Text, FilePreview.classify("Dockerfile", 300))
        assertEquals(PreviewKind.Text, FilePreview.classify(".bashrc", 300))
        assertEquals(PreviewKind.Text, FilePreview.classify("authorized_keys", 300))
        assertEquals(PreviewKind.Text, FilePreview.classify("sshd_config", 300))
    }

    @Test
    fun knownBinariesGoToOpenWith() {
        assertTrue(FilePreview.classify("backup.tar.gz", 10) is PreviewKind.Unsupported)
        assertTrue(FilePreview.classify("app.apk", 10) is PreviewKind.Unsupported)
        assertTrue(FilePreview.classify("report.pdf", null) is PreviewKind.Unsupported)
    }

    @Test
    fun unknownSmallFilesAreSniffed() {
        assertEquals(PreviewKind.Sniff, FilePreview.classify("id_ed25519.bak", 400))
        assertEquals(PreviewKind.Sniff, FilePreview.classify("data", null))
        assertTrue(FilePreview.classify("data", FilePreview.TEXT_LIMIT + 1) is PreviewKind.Unsupported)
    }

    @Test
    fun sizeLimitsApplyPerKind() {
        assertEquals(PreviewKind.Text, FilePreview.classify("big.log", FilePreview.TEXT_LIMIT))
        val tooBigText = FilePreview.classify("big.log", FilePreview.TEXT_LIMIT + 1)
        assertTrue(tooBigText is PreviewKind.Unsupported)
        assertTrue((tooBigText as PreviewKind.Unsupported).reason.contains("1 MiB"))

        assertEquals(PreviewKind.Image, FilePreview.classify("big.png", FilePreview.IMAGE_LIMIT))
        val tooBigImage = FilePreview.classify("big.png", FilePreview.IMAGE_LIMIT + 1)
        assertTrue(tooBigImage is PreviewKind.Unsupported)
        assertTrue((tooBigImage as PreviewKind.Unsupported).reason.contains("16 MiB"))
    }

    @Test
    fun textDecodeIsStrictUtf8() {
        assertEquals("héllo\n", FilePreview.decodeText("héllo\n".toByteArray(Charsets.UTF_8)))
        assertEquals("", FilePreview.decodeText(ByteArray(0)))
        assertNull(FilePreview.decodeText(byteArrayOf(0x7f, 0x45, 0x4c, 0x46, 0x00, 0x01)))
        assertNull(FilePreview.decodeText(byteArrayOf(0xC3.toByte(), 0x28)))
        assertNull(FilePreview.decodeText("héllo".toByteArray(Charsets.ISO_8859_1)))
    }

    @Test
    fun sampleSizeKeepsLongestEdgeWithinLimit() {
        assertEquals(1, FilePreview.sampleSize(1024, 768, 2048))
        assertEquals(1, FilePreview.sampleSize(2048, 100, 2048))
        assertEquals(2, FilePreview.sampleSize(4000, 3000, 2048))
        assertEquals(4, FilePreview.sampleSize(8000, 200, 2048))
        assertEquals(8, FilePreview.sampleSize(200, 12_000, 2048))
    }

    @Test
    fun previewStateTracksEdits() {
        val entry = com.termoso.core.SftpEntry(
            name = "a.txt",
            path = "/tmp/a.txt",
            kind = com.termoso.core.EntryKind.FILE,
            targetKind = com.termoso.core.EntryKind.FILE,
            isDir = false,
            size = 3u,
            mode = null,
            permissions = "-rw-r--r--",
            owner = null,
            modifiedMs = null,
            linkTarget = null,
            hidden = false,
        )
        val viewing = PreviewState(entry, loading = false, text = "abc")
        assertTrue(!viewing.editing && !viewing.dirty)
        val clean = viewing.copy(draft = "abc")
        assertTrue(clean.editing && !clean.dirty)
        val dirty = viewing.copy(draft = "abcd")
        assertTrue(dirty.editing && dirty.dirty)
    }
}
