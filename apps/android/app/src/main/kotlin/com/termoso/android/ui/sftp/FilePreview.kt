package com.termoso.android.ui.sftp

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import com.termoso.android.R
import com.termoso.android.str
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import kotlin.text.Charsets.UTF_8

/** What the in-app viewer can do with a remote file, decided before any bytes are fetched. */
sealed interface PreviewKind {
    data object Text : PreviewKind
    data object Image : PreviewKind

    /** Extension says nothing; fetch up to [FilePreview.TEXT_LIMIT] and sniff. */
    data object Sniff : PreviewKind

    /** Hand it to another app ("Open with"). */
    data class Unsupported(val reason: String) : PreviewKind
}

/** File-type and size policy for the SFTP viewer/editor; UI-free so it is unit-testable. */
object FilePreview {
    /** Largest file the text viewer/editor loads; bigger ones go to "Edit in terminal" / Download. */
    const val TEXT_LIMIT: Long = 1L shl 20

    /** Largest encoded image the viewer decodes. */
    const val IMAGE_LIMIT: Long = 16L shl 20

    /** Longest edge of the decoded bitmap, so a huge photo does not blow the heap. */
    const val IMAGE_MAX_EDGE = 2048

    private val imageExt = setOf("png", "jpg", "jpeg", "gif", "webp", "bmp", "heic", "heif", "avif")

    private val textExt = setOf(
        "txt", "md", "markdown", "rst", "log", "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "ini", "cfg",
        "conf", "config", "env", "properties", "xml", "html", "htm", "css", "scss", "js", "mjs", "ts", "tsx", "jsx",
        "sh", "bash", "zsh", "fish", "py", "rb", "pl", "php", "go", "rs", "c", "h", "cc", "cpp", "hpp", "java", "kt",
        "kts", "gradle", "swift", "sql", "lua", "vim", "service", "socket", "timer", "unit", "list", "sum", "mod",
        "lock", "diff", "patch", "pem", "pub", "crt", "csr", "key", "gitignore", "dockerignore", "editorconfig",
    )

    /** Well-known extensionless text files under `/etc` and dotfiles. */
    private val textNames = setOf(
        "dockerfile", "makefile", "cmakelists.txt", "readme", "license", "changelog", "authors", "todo", "hosts",
        "passwd", "group", "fstab", "crontab", "profile", "bashrc", "zshrc", "vimrc", "gitconfig", "authorized_keys",
        "known_hosts", "config", "motd", "issue", "hostname", "resolv.conf", "sshd_config", "ssh_config",
    )

    private val binaryExt = setOf(
        "zip", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "tar", "deb", "rpm", "apk", "jar", "war", "iso", "img",
        "bin", "exe", "dll", "so", "dylib", "o", "a", "class", "pyc", "wasm", "pdf", "doc", "docx", "xls", "xlsx",
        "ppt", "pptx", "odt", "ods", "mp3", "ogg", "flac", "wav", "m4a", "mp4", "mkv", "mov", "avi", "webm", "ttf",
        "otf", "woff", "woff2", "sqlite", "db", "dat",
    )

    fun classify(name: String, size: Long?): PreviewKind {
        val lower = name.lowercase()
        val ext = lower.substringAfterLast('.', "")
        return when {
            ext in imageExt -> if (size != null && size > IMAGE_LIMIT) {
                PreviewKind.Unsupported(str(R.string.image_is_larger_than_open_it_with_another, mib(IMAGE_LIMIT)))
            } else {
                PreviewKind.Image
            }
            ext in textExt || lower.trimStart('.') in textNames -> if (size != null && size > TEXT_LIMIT) {
                PreviewKind.Unsupported(str(R.string.file_is_larger_than_use_edit_in_terminal, mib(TEXT_LIMIT)))
            } else {
                PreviewKind.Text
            }
            ext in binaryExt -> PreviewKind.Unsupported(str(R.string.not_a_text_or_image_file))
            size != null && size > TEXT_LIMIT -> PreviewKind.Unsupported(str(R.string.file_is_larger_than, mib(TEXT_LIMIT)))
            else -> PreviewKind.Sniff
        }
    }

    /**
     * Strict UTF-8 decode of [bytes]; `null` when it has NUL bytes or invalid sequences,
     * i.e. it is not something the text editor should round-trip.
     */
    fun decodeText(bytes: ByteArray): String? {
        val probe = minOf(bytes.size, 8192)
        for (i in 0 until probe) if (bytes[i] == 0.toByte()) return null
        val decoder = UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
        return try {
            decoder.decode(ByteBuffer.wrap(bytes)).toString()
        } catch (_: CharacterCodingException) {
            null
        }
    }

    /** Decode with power-of-two subsampling so the longest edge stays within [IMAGE_MAX_EDGE]. */
    fun decodeImage(bytes: ByteArray): Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
        val opts = BitmapFactory.Options().apply {
            inSampleSize = sampleSize(bounds.outWidth, bounds.outHeight, IMAGE_MAX_EDGE)
        }
        return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
    }

    fun sampleSize(width: Int, height: Int, maxEdge: Int): Int {
        var sample = 1
        while (maxOf(width, height) / sample > maxEdge) sample *= 2
        return sample
    }

    private fun mib(bytes: Long) = "${bytes shr 20} MiB"
}

/** One file open in the viewer/editor. */
data class PreviewState(
    val entry: com.termoso.core.SftpEntry,
    val loading: Boolean = true,
    val text: String? = null,
    val image: Bitmap? = null,
    /** Editor buffer; `null` while just viewing. */
    val draft: String? = null,
    val saving: Boolean = false,
    val error: String? = null,
) {
    val editing: Boolean get() = draft != null
    val dirty: Boolean get() = draft != null && draft != text
}
