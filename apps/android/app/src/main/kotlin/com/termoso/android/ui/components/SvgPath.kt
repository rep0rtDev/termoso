package com.termoso.android.ui.components

/**
 * Rewrites SVG path data into a fully separated token form that Compose's
 * `addPathNodes` parses faithfully. Minified SVGs (Simple Icons and friends)
 * pack the two arc flags together (`a1 1 0 01-.4.1`), which Compose reads as
 * a single number and silently drops the following arc arguments.
 */
object SvgPath {
    private val ARGC = mapOf(
        'M' to 2, 'L' to 2, 'H' to 1, 'V' to 1, 'C' to 6, 'S' to 4, 'Q' to 4, 'T' to 2, 'A' to 7, 'Z' to 0,
    )

    fun normalize(d: String): String {
        val out = StringBuilder(d.length + 64)
        var i = 0
        var cmd = 'M'
        var argIndex = 0
        var argc = 2
        fun skipSeparators() {
            while (i < d.length && (d[i] == ' ' || d[i] == ',' || d[i] == '\t' || d[i] == '\n' || d[i] == '\r')) i++
        }
        while (true) {
            skipSeparators()
            if (i >= d.length) break
            val c = d[i]
            if (c.isLetter()) {
                cmd = c
                argc = ARGC[c.uppercaseChar()] ?: throw IllegalArgumentException("unknown command '$c' in path data")
                argIndex = 0
                if (out.isNotEmpty()) out.append(' ')
                out.append(c)
                i++
                if (argc == 0) continue
            } else {
                if (argIndex == argc) {
                    argIndex = 0
                    if (cmd == 'M') cmd = 'L' else if (cmd == 'm') cmd = 'l'
                    if (argc == 0) throw IllegalArgumentException("numbers after Z in path data")
                }
                out.append(' ')
                if (cmd.uppercaseChar() == 'A' && (argIndex == 3 || argIndex == 4)) {
                    require(c == '0' || c == '1') { "bad arc flag '$c' in path data" }
                    out.append(c)
                    i++
                } else {
                    val start = i
                    if (d[i] == '+' || d[i] == '-') i++
                    var dot = false
                    while (i < d.length) {
                        val ch = d[i]
                        if (ch.isDigit()) i++
                        else if (ch == '.' && !dot) { dot = true; i++ }
                        else break
                    }
                    if (i < d.length && (d[i] == 'e' || d[i] == 'E')) {
                        i++
                        if (i < d.length && (d[i] == '+' || d[i] == '-')) i++
                        while (i < d.length && d[i].isDigit()) i++
                    }
                    require(i > start && d.substring(start, i) !in setOf("+", "-", ".", "+.", "-.")) {
                        "bad number at $start in path data"
                    }
                    out.append(d, start, i)
                }
                argIndex++
            }
        }
        return out.toString()
    }
}
