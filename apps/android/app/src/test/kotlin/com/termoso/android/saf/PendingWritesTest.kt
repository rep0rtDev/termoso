package com.termoso.android.saf

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

class PendingWritesTest {
    private val host = "0f6b3f0a-4d0e-4c8e-9a2b-1d3e5f7a9b0c"
    private val doc = "$host+webdav:/home/u/db.kdbx"
    private val other = "$host+webdav:/home/u/other.txt"

    @Test
    fun nothingPendingReturnsAtOnce() {
        val p = PendingWrites(openGraceMs = 10_000, releaseLimitMs = 10_000)
        val t0 = System.nanoTime()
        assertTrue(p.await(doc))
        assertTrue(System.nanoTime() - t0 < TimeUnit.SECONDS.toNanos(1))
    }

    @Test
    fun queryWaitsForReleaseToFinish() {
        val p = PendingWrites(openGraceMs = 50, releaseLimitMs = 10_000)
        p.opened(doc)
        p.releasing(doc)
        val committed = CountDownLatch(1)
        val worker = thread {
            Thread.sleep(300)
            committed.countDown()
            p.released(doc)
        }
        assertTrue(p.await(doc))
        assertEquals(0, committed.count)
        worker.join()
    }

    @Test
    fun releaseStartedDuringGraceExtendsTheWait() {
        val p = PendingWrites(openGraceMs = 100, releaseLimitMs = 10_000)
        p.opened(doc)
        val worker = thread {
            Thread.sleep(50)
            p.releasing(doc)
            Thread.sleep(300)
            p.released(doc)
        }
        assertTrue(p.await(doc))
        worker.join()
    }

    @Test
    fun handleKeptOpenOnlyDelaysForTheGrace() {
        var now = 0L
        val p = PendingWrites(openGraceMs = 2_000, releaseLimitMs = 60_000, clock = { now })
        p.opened(doc)
        val worker = thread {
            Thread.sleep(100)
            now = 2_001
            p.released(other) // any settle signal re-checks the deadline
        }
        assertFalse(p.await(doc))
        worker.join()
    }

    @Test
    fun onlyMatchingDocumentsAreWaitedFor() {
        val p = PendingWrites(openGraceMs = 10_000, releaseLimitMs = 10_000)
        p.opened(other)
        assertTrue(p.await(doc))
        assertTrue(p.await { DocumentId.parse(it)?.path?.endsWith(".kdbx") == true })
        p.releasing(other)
        p.released(other)
        assertTrue(p.await(other))
    }

    @Test
    fun secondHandleKeepsDocumentPendingUntilBothRelease() {
        val p = PendingWrites(openGraceMs = 50, releaseLimitMs = 10_000)
        p.opened(doc)
        p.opened(doc)
        p.releasing(doc)
        p.released(doc)
        assertFalse(p.await(doc))
        p.releasing(doc)
        p.released(doc)
        assertTrue(p.await(doc))
    }
}
