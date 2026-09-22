package com.termoso.android.saf

import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * Documents with a write in flight, so metadata queries can wait for it.
 *
 * A writer's `close()` returns before the kernel delivers the descriptor's
 * release, and for protocols that commit on close (WebDAV spools a PUT; some
 * SFTP servers publish on close) the remote file is only final once that
 * release has run. An app that closes a file and immediately asks for its
 * size or modification time would otherwise read the previous version.
 *
 * Two phases per document: [open] handles, waited for only briefly (a handle
 * an app keeps open indefinitely must not stall the Files UI), and
 * [releasing] handles, waited for as long as an upload plausibly takes.
 */
class PendingWrites(
    private val openGraceMs: Long = 2_000,
    private val releaseLimitMs: Long = 60_000,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    private class State(var open: Int = 0, var releasing: Int = 0)

    private val lock = ReentrantLock()
    private val settled = lock.newCondition()
    private val states = HashMap<String, State>()

    /** A writable handle on [documentId] was handed out. */
    fun opened(documentId: String) = lock.withLock {
        states.getOrPut(documentId) { State() }.open++
    }

    /** That handle was released by the app; its data is now being committed. */
    fun releasing(documentId: String) = lock.withLock {
        val s = states[documentId] ?: return@withLock
        if (s.open > 0) s.open--
        s.releasing++
    }

    /** The commit for one released handle finished (or failed). */
    fun released(documentId: String) = lock.withLock {
        states[documentId]?.let { s ->
            if (s.releasing > 0) s.releasing--
            if (s.open == 0 && s.releasing == 0) states.remove(documentId)
        }
        settled.signalAll()
    }

    /**
     * Block until no write on a document accepted by [matches] is pending, or
     * the phase-dependent limit passes. Returns whether everything settled.
     */
    fun await(matches: (String) -> Boolean): Boolean {
        lock.withLock {
            val start = clock()
            while (true) {
                val pending = states.filterKeys(matches).values
                if (pending.isEmpty()) return true
                val limit = if (pending.any { it.releasing > 0 }) releaseLimitMs else openGraceMs
                val remaining = start + limit - clock()
                if (remaining <= 0) return false
                settled.await(remaining, TimeUnit.MILLISECONDS)
            }
        }
    }

    /** [await] for exactly [documentId]. */
    fun await(documentId: String): Boolean = await { it == documentId }
}
