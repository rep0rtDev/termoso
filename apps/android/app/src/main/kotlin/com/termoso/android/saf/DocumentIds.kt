package com.termoso.android.saf

import android.provider.DocumentsContract
import android.webkit.MimeTypeMap

/**
 * Document ids handed to other apps: `<host uuid>:<absolute remote path>`.
 * The uuid is the saved host; the path is the remote path as-is (Unicode,
 * spaces and colons included), always absolute and normalized. The empty
 * path is the host's root document — the remote home directory, resolved
 * only once connected.
 *
 * Ids come back from arbitrary apps, so every one is parsed strictly here
 * and nothing about the remote path is inferred from URI text.
 */
data class DocumentId(val hostId: String, val path: String) {
    /** The host itself, listed as its home directory. */
    val isRoot: Boolean get() = path.isEmpty()

    /** Last path component; empty for the root. */
    val name: String get() = path.substringAfterLast('/')

    /** Id of the enclosing directory, or `null` for the root and for `/`. */
    val parent: DocumentId?
        get() = when {
            isRoot || path == "/" -> null
            else -> DocumentId(hostId, path.substringBeforeLast('/').ifEmpty { "/" })
        }

    fun child(name: String): DocumentId {
        require(isValidName(name)) { "bad name" }
        return DocumentId(hostId, joinPath(path.ifEmpty { "/" }, name))
    }

    /** True when [other] lives anywhere under this directory (the root spans the host). */
    fun contains(other: DocumentId): Boolean = when {
        other.hostId != hostId -> false
        isRoot -> true
        path == "/" -> other.path.length > 1
        else -> other.path.startsWith("$path/")
    }

    fun encode(): String = "$hostId:$path"

    companion object {
        private val UUID_RE = Regex("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")

        fun root(hostId: String): DocumentId = DocumentId(hostId, "")

        /** Parse an id from another app; `null` for anything malformed or escaping its path. */
        fun parse(raw: String?): DocumentId? {
            if (raw == null) return null
            val sep = raw.indexOf(':')
            if (sep < 0) return null
            val host = raw.substring(0, sep)
            if (!UUID_RE.matches(host)) return null
            val path = raw.substring(sep + 1)
            if (path.isEmpty()) return DocumentId(host, "")
            val normalized = normalizeAbsolute(path) ?: return null
            return DocumentId(host, normalized)
        }

        /** Same, throwing the exception SAF callers expect. */
        fun parseOrThrow(raw: String?): DocumentId =
            parse(raw) ?: throw java.io.FileNotFoundException("Unknown document")

        /**
         * `/a//b/./c/` → `/a/b/c`. Relative paths, `..` segments and NULs are
         * rejected rather than resolved: an id never gets to point above where
         * its parent said it lives.
         */
        fun normalizeAbsolute(path: String): String? {
            if (!path.startsWith('/') || path.contains('\u0000')) return null
            val parts = path.split('/').filter { it.isNotEmpty() && it != "." }
            if (parts.any { it == ".." }) return null
            return "/" + parts.joinToString("/")
        }

        /** A single path component another app may name a new file or folder. */
        fun isValidName(name: String): Boolean =
            name.isNotEmpty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\u0000') &&
                name.length <= 255

        fun joinPath(dir: String, name: String): String = if (dir == "/") "/$name" else "$dir/$name"
    }
}

/** Root ids: the host uuid, or [LOCKED] while the vault cannot be opened silently. */
object RootIds {
    const val LOCKED = "locked"

    /** Document id behind the placeholder root; never a valid [DocumentId]. */
    const val LOCKED_DOCUMENT = "locked:"
}

/** MIME type for a remote entry by name; directories are [DocumentsContract.Document.MIME_TYPE_DIR]. */
object DocumentMime {
    const val BINARY = "application/octet-stream"

    fun of(name: String, isDir: Boolean, lookup: (String) -> String? = ::systemLookup): String {
        if (isDir) return DocumentsContract.Document.MIME_TYPE_DIR
        val ext = name.substringAfterLast('.', "").lowercase()
        if (ext.isEmpty() || ext == name.lowercase()) return BINARY
        return lookup(ext) ?: BINARY
    }

    private fun systemLookup(ext: String): String? = MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext)
}

/**
 * `name`, `name (1)`, `name (2)`… — the first not in [taken], keeping the
 * extension: `report.pdf` → `report (1).pdf`.
 */
fun uniqueName(name: String, taken: Set<String>): String {
    if (name !in taken) return name
    val dot = name.lastIndexOf('.')
    val (stem, ext) = if (dot > 0) name.substring(0, dot) to name.substring(dot) else name to ""
    var n = 1
    while (true) {
        val candidate = "$stem ($n)$ext"
        if (candidate !in taken) return candidate
        n++
    }
}
