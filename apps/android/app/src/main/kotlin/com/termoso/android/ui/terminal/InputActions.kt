package com.termoso.android.ui.terminal

import com.termoso.core.KeyGroup
import com.termoso.core.KeyMods
import com.termoso.core.PanelKeyDef
import com.termoso.core.SpecialKey

/**
 * The string grammar shared by custom key-panel buttons and volume-key
 * bindings, as stored in `MobileSettings` (see `PanelKeyDef` in Rust):
 *
 * ```
 * [ctrl+][alt+][shift+] key:<SPECIAL> | text:<literal> | mod:ctrl|alt|shift
 * ```
 *
 * Parsing is lenient: anything unrecognised yields `null` and the button is
 * skipped, so a layout synced from a newer build never breaks the panel.
 */
object KeyActions {
    private val NONE = KeyMods(ctrl = false, alt = false, shift = false)

    fun parse(def: PanelKeyDef): PanelKey? = parse(def.action, def.label)

    fun parse(action: String, label: String? = null): PanelKey? {
        var rest = action.trimStart() // trailing whitespace is payload for `text: `
        var ctrl = false
        var alt = false
        var shift = false
        while (true) {
            val lower = rest.lowercase()
            when {
                lower.startsWith("ctrl+") -> { ctrl = true; rest = rest.drop(5) }
                lower.startsWith("alt+") -> { alt = true; rest = rest.drop(4) }
                lower.startsWith("shift+") -> { shift = true; rest = rest.drop(6) }
                else -> break
            }
        }
        val mods = KeyMods(ctrl = ctrl, alt = alt, shift = shift)
        val sep = rest.indexOf(':')
        if (sep <= 0) return null
        val kind = rest.substring(0, sep).lowercase()
        val arg = rest.substring(sep + 1)
        return when (kind) {
            "key" -> {
                val key = SpecialKey.entries.firstOrNull { it.name.equals(arg.trim(), ignoreCase = true) } ?: return null
                PanelKey.Special(label?.ifBlank { null } ?: defaultLabel(key, mods), key, mods)
            }
            "text" -> {
                if (arg.isEmpty() || ctrl && arg.length != 1) return null
                PanelKey.Text(label?.ifBlank { null } ?: defaultLabel(arg, mods), arg, mods)
            }
            "mod" -> when (arg.trim().lowercase()) {
                "ctrl" -> PanelKey.Modifier(label?.ifBlank { null } ?: "Ctrl", StickyMod.CTRL)
                "alt" -> PanelKey.Modifier(label?.ifBlank { null } ?: "Alt", StickyMod.ALT)
                "shift" -> PanelKey.Modifier(label?.ifBlank { null } ?: "Shift", StickyMod.SHIFT)
                else -> null
            }
            else -> null
        }
    }

    fun encode(key: PanelKey): String = when (key) {
        is PanelKey.Special -> prefix(key.mods) + "key:" + key.key.name
        is PanelKey.Text -> prefix(key.mods) + "text:" + key.text
        is PanelKey.Modifier -> "mod:" + key.which.name.lowercase()
    }

    fun toDef(key: PanelKey): PanelKeyDef = PanelKeyDef(label = key.label, action = encode(key))

    private fun prefix(mods: KeyMods): String =
        (if (mods.ctrl) "ctrl+" else "") + (if (mods.alt) "alt+" else "") + (if (mods.shift) "shift+" else "")

    private fun modLabel(mods: KeyMods): String =
        (if (mods.ctrl) "^" else "") + (if (mods.alt) "⌥" else "") + (if (mods.shift) "⇧" else "")

    fun defaultLabel(key: SpecialKey, mods: KeyMods = NONE): String = modLabel(mods) + when (key) {
        SpecialKey.LEFT -> "←"
        SpecialKey.UP -> "↑"
        SpecialKey.DOWN -> "↓"
        SpecialKey.RIGHT -> "→"
        SpecialKey.ESCAPE -> "Esc"
        SpecialKey.ENTER -> "Enter"
        SpecialKey.BACKSPACE -> "⌫"
        SpecialKey.DELETE -> "Delete"
        SpecialKey.INSERT -> "Insert"
        SpecialKey.HOME -> "Home"
        SpecialKey.END -> "End"
        SpecialKey.PAGE_UP -> "Pg Up"
        SpecialKey.PAGE_DOWN -> "Pg Dn"
        SpecialKey.TAB -> "Tab"
        else -> key.name
    }

    fun defaultLabel(text: String, mods: KeyMods = NONE): String {
        val body = when (text) {
            " " -> "Space"
            "\n" -> "Enter"
            else -> if (mods.ctrl) text.uppercase() else text
        }
        return modLabel(mods) + body
    }

    /** Human description for pickers ("Ctrl+C", "Shift+Tab", "Type |"). */
    fun describe(key: PanelKey): String = when (key) {
        is PanelKey.Modifier -> "Sticky ${key.label}"
        is PanelKey.Special -> words(key.mods) + when (key.key) {
            SpecialKey.LEFT -> "Left"
            SpecialKey.UP -> "Up"
            SpecialKey.DOWN -> "Down"
            SpecialKey.RIGHT -> "Right"
            SpecialKey.BACKSPACE -> "Backspace"
            else -> defaultLabel(key.key)
        }
        is PanelKey.Text -> when {
            key.mods.ctrl || key.mods.alt -> words(key.mods) + key.text.uppercase()
            key.text == " " -> "Space"
            else -> "Type ${key.text}"
        }
    }

    private fun words(mods: KeyMods): String =
        (if (mods.ctrl) "Ctrl+" else "") + (if (mods.alt) "Alt+" else "") + (if (mods.shift) "Shift+" else "")
}

/** Something a volume key can do besides sending a key. */
enum class UiAction(val id: String, val title: String) {
    FONT_UP("font_up", "Increase text size"),
    FONT_DOWN("font_down", "Decrease text size"),
    SCROLL_UP("scroll_up", "Scroll up"),
    SCROLL_DOWN("scroll_down", "Scroll down"),
    NEXT_SESSION("next_session", "Next session"),
    PREV_SESSION("prev_session", "Previous session"),
    TOGGLE_KEYBOARD("toggle_keyboard", "Show / hide keyboard"),
    CLOSE_SESSION("close_session", "Close session"),
}

/** A volume-key binding: nothing, an app action, or a key to send. */
sealed interface InputAction {
    data object Disabled : InputAction
    data class Ui(val action: UiAction) : InputAction
    data class Key(val key: PanelKey) : InputAction

    companion object {
        fun parse(value: String): InputAction {
            val v = value.trim()
            if (v.isEmpty() || v.equals("disabled", ignoreCase = true)) return Disabled
            UiAction.entries.firstOrNull { it.id == v.lowercase() }?.let { return Ui(it) }
            return KeyActions.parse(v)?.let { Key(it) } ?: Disabled
        }

        fun encode(action: InputAction): String = when (action) {
            Disabled -> ""
            is Ui -> action.action.id
            is Key -> KeyActions.encode(action.key)
        }

        fun title(action: InputAction): String = when (action) {
            Disabled -> "Disabled"
            is Ui -> action.action.title
            is Key -> KeyActions.describe(action.key)
        }

        /** What the volume-key picker offers, in display order. */
        val choices: List<InputAction> by lazy {
            buildList {
                add(Disabled)
                UiAction.entries.forEach { add(Ui(it)) }
                listOf(
                    "key:ESCAPE", "key:TAB", "shift+key:TAB", "key:ENTER", "key:BACKSPACE",
                    "key:UP", "key:DOWN", "key:LEFT", "key:RIGHT", "key:PAGE_UP", "key:PAGE_DOWN",
                    "mod:ctrl", "mod:alt", "text: ",
                    "ctrl+text:c", "ctrl+text:d", "ctrl+text:z", "ctrl+text:l", "ctrl+text:r", "ctrl+text:x",
                    "text:.", "text:/", "text::", "text:?", "text:-", "text:$", "text:|",
                ).forEach { s -> KeyActions.parse(s)?.let { add(Key(it)) } }
            }
        }
    }
}

/** Built-in rows of the expandable key panel and the custom-layout resolver. */
object KeyGroups {
    private fun special(key: SpecialKey, shift: Boolean = false): PanelKey =
        PanelKey.Special(
            KeyActions.defaultLabel(key, KeyMods(ctrl = false, alt = false, shift = shift)),
            key,
            KeyMods(ctrl = false, alt = false, shift = shift),
        )

    private fun text(vararg t: String): List<PanelKey> = t.map { PanelKey.Text(KeyActions.defaultLabel(it), it) }

    val defaults: List<KeyGroup> by lazy {
        fun group(id: String, name: String, keys: List<PanelKey>) =
            KeyGroup(id = id, name = name, keys = keys.map(KeyActions::toDef), enabled = true)
        listOf(
            group(
                "arrows", "Arrows & editing",
                listOf(
                    special(SpecialKey.LEFT), special(SpecialKey.UP), special(SpecialKey.DOWN), special(SpecialKey.RIGHT),
                    PanelKey.Modifier("Alt", StickyMod.ALT), special(SpecialKey.TAB),
                    special(SpecialKey.INSERT), special(SpecialKey.DELETE),
                ),
            ),
            group(
                "nav", "Navigation",
                listOf(special(SpecialKey.HOME), special(SpecialKey.PAGE_UP), special(SpecialKey.PAGE_DOWN), special(SpecialKey.END)) +
                    text("|", "\\", "?", "-"),
            ),
            group("symbols1", "Symbols", text("/", ":", ";", "!", "~", "@", "$", "*")),
            group("symbols2", "More symbols", text("^", "%", "=", "`", "<", ">", "(", ")")),
            group(
                "brackets", "Brackets & F1–F4",
                text("{", "}", "[", "]") + listOf(SpecialKey.F1, SpecialKey.F2, SpecialKey.F3, SpecialKey.F4).map { special(it) },
            ),
            group(
                "fkeys", "F5–F12",
                listOf(
                    SpecialKey.F5, SpecialKey.F6, SpecialKey.F7, SpecialKey.F8,
                    SpecialKey.F9, SpecialKey.F10, SpecialKey.F11, SpecialKey.F12,
                ).map { special(it) },
            ),
        )
    }

    /** Groups to edit: the stored layout, or the built-ins when nothing is stored. */
    fun editable(stored: List<KeyGroup>): List<KeyGroup> = stored.ifEmpty { defaults }

    /** Rows to render: enabled groups with their parsable keys; empty rows vanish. */
    fun rows(stored: List<KeyGroup>): List<List<PanelKey>> =
        editable(stored).filter { it.enabled }
            .map { g -> g.keys.mapNotNull(KeyActions::parse) }
            .filter { it.isNotEmpty() }

    /** Everything a user may put on a custom row, in picker order. */
    val palette: List<PanelKey> by lazy {
        buildList {
            addAll(listOf(SpecialKey.LEFT, SpecialKey.UP, SpecialKey.DOWN, SpecialKey.RIGHT).map { special(it) })
            add(PanelKey.Modifier("Ctrl", StickyMod.CTRL))
            add(PanelKey.Modifier("Alt", StickyMod.ALT))
            add(PanelKey.Modifier("Shift", StickyMod.SHIFT))
            addAll(
                listOf(
                    SpecialKey.ESCAPE, SpecialKey.TAB, SpecialKey.ENTER, SpecialKey.BACKSPACE, SpecialKey.INSERT,
                    SpecialKey.DELETE, SpecialKey.HOME, SpecialKey.END, SpecialKey.PAGE_UP, SpecialKey.PAGE_DOWN,
                ).map { special(it) },
            )
            add(special(SpecialKey.TAB, shift = true))
            listOf("c", "d", "z", "l", "r", "x", "a", "e", "u", "k", "w").forEach { c ->
                KeyActions.parse("ctrl+text:$c")?.let { add(it) }
            }
            addAll(text(" ", "|", "\\", "?", "-", "/", ":", ";", "!", "~", "@", "$", "*", "^", "%", "=", "`", "<", ">", "(", ")", "{", "}", "[", "]", "#", "&", "'", "\"", ",", ".", "_", "+"))
            addAll(
                listOf(
                    SpecialKey.F1, SpecialKey.F2, SpecialKey.F3, SpecialKey.F4, SpecialKey.F5, SpecialKey.F6,
                    SpecialKey.F7, SpecialKey.F8, SpecialKey.F9, SpecialKey.F10, SpecialKey.F11, SpecialKey.F12,
                ).map { special(it) },
            )
        }
    }

    fun newGroupId(existing: List<KeyGroup>): String {
        var n = existing.size + 1
        while (existing.any { it.id == "custom$n" }) n++
        return "custom$n"
    }
}
