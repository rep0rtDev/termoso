package com.termoso.android.ui.terminal

import android.content.Context
import android.graphics.Typeface
import androidx.annotation.FontRes
import androidx.core.content.res.ResourcesCompat
import com.termoso.android.R

/** A bundled monospace family; `null` resources fall back to a synthesized style. */
class TerminalFont(
    val name: String,
    @FontRes val regular: Int?,
    @FontRes val bold: Int? = null,
    @FontRes val italic: Int? = null,
    @FontRes val boldItalic: Int? = null,
)

const val SYSTEM_MONOSPACE = "System monospace"

val terminalFonts: List<TerminalFont> = listOf(
    TerminalFont(SYSTEM_MONOSPACE, null),
    TerminalFont(
        "JetBrains Mono",
        R.font.jetbrains_mono_regular,
        R.font.jetbrains_mono_bold,
        R.font.jetbrains_mono_italic,
        R.font.jetbrains_mono_bold_italic,
    ),
    TerminalFont("Fira Code", R.font.fira_code_regular, R.font.fira_code_bold),
    TerminalFont(
        "Source Code Pro",
        R.font.source_code_pro_regular,
        R.font.source_code_pro_bold,
        R.font.source_code_pro_italic,
        R.font.source_code_pro_bold_italic,
    ),
    TerminalFont(
        "Ubuntu Mono",
        R.font.ubuntu_mono_regular,
        R.font.ubuntu_mono_bold,
        R.font.ubuntu_mono_italic,
        R.font.ubuntu_mono_bold_italic,
    ),
)

/** The four style variants the cell renderer indexes with `flags and 3`. */
class TerminalTypefaces(regular: Typeface, bold: Typeface, italic: Typeface, boldItalic: Typeface) {
    private val faces = arrayOf(regular, bold, italic, boldItalic)

    operator fun get(style: Int): Typeface = faces[style and 3]

    companion object {
        val system = TerminalTypefaces(
            Typeface.MONOSPACE,
            Typeface.create(Typeface.MONOSPACE, Typeface.BOLD),
            Typeface.create(Typeface.MONOSPACE, Typeface.ITALIC),
            Typeface.create(Typeface.MONOSPACE, Typeface.BOLD_ITALIC),
        )
    }
}

/** Resolves the family saved in settings; unknown names (e.g. synced from desktop) use the system font. */
fun terminalTypefaces(context: Context, family: String): TerminalTypefaces {
    val font = terminalFonts.firstOrNull { it.name.equals(family.trim(), ignoreCase = true) }
    val regularRes = font?.regular ?: return TerminalTypefaces.system
    val regular = ResourcesCompat.getFont(context, regularRes) ?: return TerminalTypefaces.system
    fun variant(@FontRes res: Int?, style: Int) =
        res?.let { ResourcesCompat.getFont(context, it) } ?: Typeface.create(regular, style)
    return TerminalTypefaces(
        regular,
        variant(font.bold, Typeface.BOLD),
        variant(font.italic, Typeface.ITALIC),
        variant(font.boldItalic, Typeface.BOLD_ITALIC),
    )
}
