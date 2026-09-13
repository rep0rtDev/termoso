package com.termoso.android.ui.terminal

import com.termoso.core.TerminalPalette

/** Termoso Light; the Rust default is Termoso Dark. */
private val light = TerminalPalette(
    foreground = 0x1F2328u,
    background = 0xFFFFFFu,
    cursor = 0x1A7F37u,
    ansi = listOf(
        0x24292Fu, 0xCF222Eu, 0x116329u, 0x4D2D00u, 0x0969DAu, 0x8250DFu, 0x1B7C83u, 0x6E7781u,
        0x57606Au, 0xA40E26u, 0x1A7F37u, 0x633C01u, 0x218BFFu, 0xA475F9u, 0x3192AAu, 0x8C959Fu,
    ),
)

/** Palette for a settings theme name; null lets Rust use Termoso Dark. */
fun paletteFor(theme: String): TerminalPalette? = when (theme) {
    "Termoso Light" -> light
    else -> null
}
