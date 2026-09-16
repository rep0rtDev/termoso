package com.termoso.android.ui.terminal

import com.termoso.core.SessionState
import com.termoso.core.SuggestionItem
import com.termoso.core.SuggestionKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AutocompleteTest {
    private fun item(kind: SuggestionKind, label: String, insert: String, desc: String = "") =
        SuggestionItem(kind = kind, label = label, desc = desc, insert = insert)

    @Test
    fun onlyAWritableConnectedShellGetsSuggestions() {
        val up = SessionState.Connected
        assertTrue(autocompleteAllowed(enabled = true, state = up, canWrite = true, isView = false))
        assertFalse(autocompleteAllowed(enabled = false, state = up, canWrite = true, isView = false))
        assertFalse(autocompleteAllowed(enabled = true, state = up, canWrite = false, isView = true))
        assertFalse(autocompleteAllowed(enabled = true, state = up, canWrite = true, isView = true))
        assertFalse(autocompleteAllowed(enabled = true, state = SessionState.Connecting("Authenticating…"), canWrite = true, isView = false))
        assertFalse(autocompleteAllowed(enabled = true, state = SessionState.Closed(null), canWrite = true, isView = false))
    }

    @Test
    fun acceptingNeverSendsALineBreak() {
        assertEquals("t ", suggestionInsert(item(SuggestionKind.COMMAND, "git", "t ")))
        assertEquals(" status", suggestionInsert(item(SuggestionKind.HISTORY, "git status", " status")))
        assertNull(suggestionInsert(item(SuggestionKind.SNIPPET, "cd /var", "d /var\n")))
        assertNull(suggestionInsert(item(SuggestionKind.SNIPPET, "cd /var", "d /var\r")))
    }

    @Test
    fun chipTextIsShortenedInTheMiddle() {
        val short = item(SuggestionKind.COMMAND, "ls", "s ")
        assertEquals("ls", suggestionChipText(short))
        val long = item(SuggestionKind.HISTORY, "docker run --rm -it -v /home/me/project:/app -w /app node:20 npm test", "")
        val text = suggestionChipText(long, max = 20)
        assertEquals(20, text.length)
        assertTrue(text.startsWith("docker run"))
        assertTrue(text.endsWith(" test"))
        assertTrue(text.contains("…"))
    }

    @Test
    fun chipDetailOnlyForCatalogueEntries() {
        assertEquals("list directory", suggestionChipDetail(item(SuggestionKind.COMMAND, "ls", "s ", "list directory")))
        assertEquals("Disk usage", suggestionChipDetail(item(SuggestionKind.SNIPPET, "df -h", "f -h", "Disk usage")))
        assertNull(suggestionChipDetail(item(SuggestionKind.HISTORY, "ls -la", " -la", "")))
        assertNull(suggestionChipDetail(item(SuggestionKind.PATH, "src/main.rs", "ain.rs ", "file")))
        assertNull(suggestionChipDetail(item(SuggestionKind.OPTION, "-l", "l ", "")))
    }

    @Test
    fun trackerQueriesOnlyWhenTheTypedLineChanges() {
        val tracker = AutocompleteTracker()
        var fetches = 0
        val fetch = { fetches++; listOf(item(SuggestionKind.COMMAND, "git", "t ")) }

        // Nothing typed yet: the strip is already empty, nothing asked.
        assertNull(tracker.next(null, fetch))
        assertNull(tracker.next(null, fetch))
        assertEquals(0, fetches)

        assertEquals(1, tracker.next("gi", fetch)?.size)
        assertEquals(1, fetches)
        // Late echo with the same text: no new query.
        assertNull(tracker.next("gi", fetch))
        assertEquals(1, fetches)

        assertEquals(1, tracker.next("git", fetch)?.size)
        assertEquals(2, fetches)
        // Enter pressed: the line is gone, the strip empties without a query.
        assertEquals(emptyList<SuggestionItem>(), tracker.next(null, fetch))
        assertEquals(2, fetches)
        // Only whitespace: nothing to offer, nothing asked.
        assertEquals(emptyList<SuggestionItem>(), tracker.next("   ", fetch))
        assertEquals(2, fetches)

        tracker.reset()
        assertEquals(emptyList<SuggestionItem>(), tracker.next("   ", fetch))
        assertEquals(2, fetches)
        assertEquals(1, tracker.next("gi", fetch)?.size)
        assertEquals(3, fetches)
    }
}
