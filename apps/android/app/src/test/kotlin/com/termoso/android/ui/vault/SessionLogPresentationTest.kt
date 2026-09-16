package com.termoso.android.ui.vault

import com.termoso.core.SessionLogCard
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionLogPresentationTest {
    private fun card(
        mine: Boolean = true,
        author: String? = null,
        completed: Boolean = true,
        endedAt: Long? = 90_000,
    ) = SessionLogCard(
        id = "l",
        vaultId = "v",
        hostId = "h",
        label = "prod",
        target = "root@10.0.0.1:22",
        protocol = "ssh",
        startedAt = 0,
        endedAt = endedAt,
        bytes = 2048u,
        mine = mine,
        author = author,
        completed = completed,
        downloaded = false,
        pinned = false,
        note = "",
    )

    @Test
    fun stripAnsiDropsColorsTitlesAndBells() {
        val raw = "\u001B]0;root@prod: ~\u0007\u001B[01;32mroot@prod\u001B[00m:\u001B[01;34m~\u001B[00m$ ls\r\n" +
            "\u001B[0m\u001B[01;34mbin\u001B[0m  etc\u0007\r\n"
        assertEquals("root@prod:~$ ls\nbin  etc\n", stripAnsi(raw))
    }

    @Test
    fun stripAnsiKeepsFinalRedrawOfCarriageReturnLines() {
        assertEquals("100%", stripAnsi("10%\r50%\r100%"))
        assertEquals("doneress 42%", stripAnsi("progress 42%\rdone"))
        assertEquals("done", stripAnsi("progress 42%\rdone\u001B[K"))
        assertEquals("done", stripAnsi("progress 42%\r\u001B[0Kdone"))
        assertEquals("axxxb", stripAnsi("axxx\u001B[Kb"))
        assertEquals("ab", stripAnsi("axxx\ra\u001B[Kb"))
        assertEquals("a\nb", stripAnsi("a\r\nb"))
    }

    @Test
    fun stripAnsiHandlesTwoByteEscapesAndCursorMoves() {
        assertEquals("ab", stripAnsi("\u001B=a\u001B[2J\u001B[Hb\u001B>"))
        assertEquals("", stripAnsi("\u001B[?2004h\u001B[?2004l"))
    }

    @Test
    fun durationIsHumanReadable() {
        assertEquals("5s", formatDuration(5))
        assertEquals("1m", formatDuration(90))
        assertEquals("1h 1m", formatDuration(3660))
    }

    @Test
    fun subtitleShowsAuthorForTeammatesAndRecordingState() {
        val mine = logSubtitle(card())
        assertTrue(mine.startsWith("SSH · "))
        assertTrue(mine.contains("1m"))
        assertFalse(mine.contains("recording"))

        val theirs = logSubtitle(card(mine = false, author = "kate"))
        assertTrue(theirs.contains("kate"))

        val anonymous = logSubtitle(card(mine = false, author = null))
        assertTrue(anonymous.contains("teammate"))

        val live = logSubtitle(card(completed = false, endedAt = null))
        assertTrue(live.endsWith("recording…"))
    }
}
