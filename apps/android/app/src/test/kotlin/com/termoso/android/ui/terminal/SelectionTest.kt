package com.termoso.android.ui.terminal

import com.termoso.core.CursorStyle
import com.termoso.core.GridFrame
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SelectionTest {
    private val wideSpacer = 1 shl 6
    private val hidden = 1 shl 7

    /** Builds a grid from rows of text; `flags` maps (row, col) to cell flags. */
    private fun grid(vararg lines: String, cols: Int = 12, flags: Map<Pair<Int, Int>, Int> = emptyMap()): CellGrid {
        val buf = ByteBuffer.allocate(lines.size * cols * 12).order(ByteOrder.LITTLE_ENDIAN)
        lines.forEachIndexed { row, line ->
            val cps = line.codePoints().toArray()
            for (col in 0 until cols) {
                buf.putInt(cps.getOrElse(col) { 0 })
                buf.putInt((flags[row to col] ?: 0) shl 24)
                buf.putInt(0)
            }
        }
        return CellGrid(
            GridFrame(
                cols = cols.toUShort(),
                rows = lines.size.toUShort(),
                cells = buf.array(),
                cursorCol = 0u,
                cursorRow = 0u,
                cursor = CursorStyle.BLOCK,
                displayOffset = 0u,
                history = 0u,
                background = 0u,
                mouseReporting = false,
                altScreen = false,
                bracketedPaste = false,
                appCursor = false,
            ),
        )
    }

    @Test
    fun startAndEndAreOrderedRegardlessOfAnchor() {
        val sel = TermSelection(CellPoint(3, 4), CellPoint(1, 9))
        assertEquals(CellPoint(1, 9), sel.start)
        assertEquals(CellPoint(3, 4), sel.end)
        assertEquals(CellPoint(0, 2), TermSelection(CellPoint(0, 7), CellPoint(0, 2)).start)
        assertEquals(CellPoint(-4, 9), sel.shiftedRows(-5).start)
    }

    @Test
    fun wordAtStopsAtSeparatorsAndBlankCells() {
        val g = grid("echo \"foo\" bar")
        assertEquals(TermSelection(CellPoint(0, 0), CellPoint(0, 3)), g.wordAt(CellPoint(0, 2)))
        assertEquals(TermSelection(CellPoint(0, 6), CellPoint(0, 8)), g.wordAt(CellPoint(0, 7)))
        assertEquals(TermSelection(CellPoint(0, 4), CellPoint(0, 4)), g.wordAt(CellPoint(0, 4)))
        assertNull(g.wordAt(CellPoint(1, 0)))
        assertEquals("foo", g.textIn(g.wordAt(CellPoint(0, 7))!!))
    }

    @Test
    fun textInSpansRowsTrimsTrailingBlanksAndClampsOutsideRows() {
        val g = grid("first line", "second", "third")
        assertEquals("line\nsecond\nth", g.textIn(TermSelection(CellPoint(0, 6), CellPoint(2, 1))))
        assertEquals("first line\nsecond\nthird", g.textIn(g.all()))
        assertEquals("first line\nsecond", g.textIn(TermSelection(CellPoint(-4, 3), CellPoint(1, 11))))
        assertEquals("", g.textIn(TermSelection(CellPoint(0, 11), CellPoint(0, 11))))
    }

    @Test
    fun wideSpacersAreDroppedAndConcealedTextIsBlanked() {
        val g = grid(
            "日\u0000本\u0000 x",
            "pw secret",
            flags = mapOf(
                (0 to 1) to wideSpacer,
                (0 to 3) to wideSpacer,
                (1 to 3) to hidden, (1 to 4) to hidden, (1 to 5) to hidden,
                (1 to 6) to hidden, (1 to 7) to hidden, (1 to 8) to hidden,
            ),
        )
        assertEquals("日本 x", g.textIn(TermSelection(CellPoint(0, 0), CellPoint(0, 5))))
        assertEquals(TermSelection(CellPoint(0, 0), CellPoint(0, 3)), g.wordAt(CellPoint(0, 2)))
        assertEquals("pw", g.textIn(TermSelection(CellPoint(1, 0), CellPoint(1, 11))))
        assertEquals(TermSelection(CellPoint(1, 5), CellPoint(1, 5)), g.wordAt(CellPoint(1, 5)))
    }
}
