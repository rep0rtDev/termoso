package com.termoso.android.ui.components

import androidx.compose.ui.graphics.vector.PathNode
import androidx.compose.ui.graphics.vector.addPathNodes
import org.junit.Assert.assertEquals
import org.junit.Test

class SvgPathTest {
    @Test
    fun expandsPackedArcFlags() {
        assertEquals(
            "a 3.001 3.001 0 0 1 -.99 .05 m 2.14 -.53",
            SvgPath.normalize("a3.001 3.001 0 01-.99.05m2.14-.53"),
        )
        assertEquals("A 1 1 0 1 1 2 2", SvgPath.normalize("A1 1 0 11 2 2"))
    }

    @Test
    fun splitsImplicitNumberBoundaries() {
        assertEquals("M 1.5 .5 l -.2 -.3 1e-3 2 z", SvgPath.normalize("M1.5.5l-.2-.3 1e-3,2z"))
        assertEquals("m 1 2 3 4 z", SvgPath.normalize("m1,2 3,4z"))
    }

    @Test
    fun composeParsesEveryArcOfEveryIcon() {
        for (icon in DistroIcons.all) {
            val normalized = SvgPath.normalize(icon.path)
            val nodes = addPathNodes(normalized)
            val arcs = nodes.count { it is PathNode.ArcTo || it is PathNode.RelativeArcTo }
            assertEquals("${icon.id}: arcs", countArcs(normalized), arcs)
            assertEquals("${icon.id}: nodes", countNodes(normalized), nodes.size)
        }
    }

    @Test
    fun rawPackedPathsLoseArcsWithoutNormalization() {
        val alpine = DistroIcons.all.first { it.id == "alpine" }
        val raw = addPathNodes(alpine.path).size
        val fixed = addPathNodes(SvgPath.normalize(alpine.path)).size
        check(fixed > raw) { "Compose now parses packed flags; SvgPath.normalize may be redundant" }
    }

    private fun countNodes(normalized: String): Int {
        var count = 0
        var cmd = 'M'
        var pending = 0
        for (tok in normalized.split(' ')) {
            if (tok.length == 1 && tok[0].isLetter()) {
                cmd = tok[0]
                pending = argc(cmd)
                if (pending == 0) count++
                continue
            }
            if (pending == 0) {
                if (cmd == 'M') cmd = 'L' else if (cmd == 'm') cmd = 'l'
                pending = argc(cmd)
            }
            pending--
            if (pending == 0) count++
        }
        return count
    }

    private fun countArcs(normalized: String): Int {
        var count = 0
        var cmd = 'M'
        var pending = 0
        for (tok in normalized.split(' ')) {
            if (tok.length == 1 && tok[0].isLetter()) {
                cmd = tok[0]
                pending = argc(cmd)
                continue
            }
            if (pending == 0) {
                if (cmd == 'M') cmd = 'L' else if (cmd == 'm') cmd = 'l'
                pending = argc(cmd)
            }
            pending--
            if (pending == 0 && cmd.uppercaseChar() == 'A') count++
        }
        return count
    }

    private fun argc(cmd: Char) = when (cmd.uppercaseChar()) {
        'M', 'L', 'T' -> 2
        'H', 'V' -> 1
        'C' -> 6
        'S', 'Q' -> 4
        'A' -> 7
        else -> 0
    }
}
