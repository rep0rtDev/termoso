package com.termoso.android.ui.terminal

import android.content.Context
import android.net.Uri
import android.webkit.MimeTypeMap
import com.termoso.android.R
import com.termoso.android.data.TerminalSession
import com.termoso.android.str
import com.termoso.android.ui.sftp.LocalFiles
import com.termoso.core.FileDropListener
import com.termoso.core.Transport
import java.io.File
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** Progress of one file being dropped into a terminal. */
data class DropProgress(val name: String, val index: Int, val count: Int, val done: Long, val total: Long?) {
    val fraction: Float? get() = total?.takeIf { it > 0 }?.let { (done.toDouble() / it).toFloat().coerceIn(0f, 1f) }
}

/**
 * Sends shared/picked files into a terminal: each content URI is copied to an
 * app-owned scratch file, handed to Rust (which uploads it over the session's
 * own SFTP channel and types the remote path), then deleted. Content URIs never
 * cross into Rust; nothing about the file is logged.
 */
object FileDrop {
    /** Why a terminal cannot take files; `null` when it can. */
    fun blocker(session: TerminalSession?): String? = when {
        session == null -> str(R.string.open_an_ssh_terminal_first)
        session.isView -> str(R.string.a_shared_terminal_you_are_viewing_cannot_receive)
        session.local != null -> str(R.string.the_local_shell_has_the_file_already_share)
        !session.rust.canSendFiles() -> when (session.transport) {
            Transport.MOSH -> str(R.string.files_cant_be_sent_over_mosh_reconnect_with)
            Transport.TELNET -> str(R.string.files_cant_be_sent_over_telnet)
            else -> str(R.string.this_terminal_has_no_ssh_connection_to_send)
        }
        else -> null
    }

    /** Sequentially upload [uris]; returns the remote paths in order. Throws on the first failure. */
    suspend fun send(
        context: Context,
        session: TerminalSession,
        uris: List<Uri>,
        onProgress: (DropProgress) -> Unit,
    ): List<String> = withContext(Dispatchers.IO) {
        val resolver = context.contentResolver
        val dir = File(context.cacheDir, "drops").apply { mkdirs() }
        val out = ArrayList<String>(uris.size)
        uris.forEachIndexed { i, uri ->
            val doc = LocalFiles.describe(resolver, uri)
            val name = withExtension(doc.name, resolver.getType(uri))
            val scratch = File(dir, UUID.randomUUID().toString())
            try {
                onProgress(DropProgress(name, i, uris.size, 0, doc.size))
                LocalFiles.copyIn(resolver, uri, scratch)
                val listener = object : FileDropListener {
                    override fun onProgress(done: ULong, total: ULong) {
                        onProgress(DropProgress(name, i, uris.size, done.toLong(), total.toLong().takeIf { it > 0 } ?: doc.size))
                    }
                }
                out += session.rust.sendFile(scratch.absolutePath, name, listener)
            } finally {
                scratch.delete()
            }
        }
        out
    }

    /** Keyboard/gallery shares often come as `image` with no extension; add one from the MIME type so tools recognise it. */
    internal fun withExtension(name: String, mime: String?): String {
        if (name.substringAfterLast('/').contains('.')) return name
        val ext = mime?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it) } ?: return name
        return "$name.$ext"
    }
}
