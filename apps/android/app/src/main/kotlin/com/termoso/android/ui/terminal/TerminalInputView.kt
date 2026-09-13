package com.termoso.android.ui.terminal

import android.content.Context
import android.text.InputType
import android.view.KeyCharacterMap
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import com.termoso.core.KeyMods
import com.termoso.core.SpecialKey

/**
 * Invisible focus target that owns the soft-keyboard connection and hardware
 * key events. `TYPE_NULL` puts IMEs into raw mode: they commit text as typed
 * (no composing/autocorrect) and send Enter/Backspace as key events, which is
 * what a terminal wants.
 */
class TerminalInputView(context: Context) : View(context) {
    var onText: (String) -> Unit = {}
    var onKey: (SpecialKey, KeyMods) -> Unit = { _, _ -> }
    /** Ctrl/Alt from hardware keyboards apply on top of the on-screen sticky ones. */
    var onTextWithMods: (String, KeyMods) -> Unit = { text, _ -> onText(text) }

    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun onCheckIsTextEditor(): Boolean = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = InputType.TYPE_NULL
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_ACTION_NONE
        return object : BaseInputConnection(this, false) {
            override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
                deliver(text.toString())
                return true
            }

            override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
                // Some IMEs still compose in TYPE_NULL; treat it as committed input.
                deliver(text.toString())
                return true
            }

            override fun finishComposingText(): Boolean = true

            override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                repeat(beforeLength) { onKey(SpecialKey.BACKSPACE, NO_MODS) }
                repeat(afterLength) { onKey(SpecialKey.DELETE, NO_MODS) }
                return true
            }

            override fun sendKeyEvent(event: KeyEvent): Boolean {
                if (event.action == KeyEvent.ACTION_DOWN) dispatchKeyEvent(event)
                return true
            }
        }
    }

    private fun deliver(text: String) {
        if (text.isEmpty()) return
        val parts = text.split('\n')
        parts.forEachIndexed { i, part ->
            if (part.isNotEmpty()) onText(part)
            if (i < parts.lastIndex) onKey(SpecialKey.ENTER, NO_MODS)
        }
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val mods = KeyMods(ctrl = event.isCtrlPressed, alt = event.isAltPressed, shift = event.isShiftPressed)
        specialKeyFor(keyCode)?.let {
            onKey(it, mods)
            return true
        }
        if (keyCode == KeyEvent.KEYCODE_SHIFT_LEFT || keyCode == KeyEvent.KEYCODE_SHIFT_RIGHT ||
            keyCode == KeyEvent.KEYCODE_CTRL_LEFT || keyCode == KeyEvent.KEYCODE_CTRL_RIGHT ||
            keyCode == KeyEvent.KEYCODE_ALT_LEFT || keyCode == KeyEvent.KEYCODE_ALT_RIGHT
        ) {
            return super.onKeyDown(keyCode, event)
        }
        val meta = event.metaState and (KeyEvent.META_CTRL_MASK or KeyEvent.META_ALT_MASK).inv()
        val ch = event.getUnicodeChar(meta)
        if (ch == 0 || ch and KeyCharacterMap.COMBINING_ACCENT != 0) return super.onKeyDown(keyCode, event)
        onTextWithMods(String(Character.toChars(ch)), KeyMods(ctrl = mods.ctrl, alt = mods.alt, shift = false))
        return true
    }

    private fun specialKeyFor(keyCode: Int): SpecialKey? = when (keyCode) {
        KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> SpecialKey.ENTER
        KeyEvent.KEYCODE_TAB -> SpecialKey.TAB
        KeyEvent.KEYCODE_DEL -> SpecialKey.BACKSPACE
        KeyEvent.KEYCODE_FORWARD_DEL -> SpecialKey.DELETE
        KeyEvent.KEYCODE_ESCAPE -> SpecialKey.ESCAPE
        KeyEvent.KEYCODE_DPAD_UP -> SpecialKey.UP
        KeyEvent.KEYCODE_DPAD_DOWN -> SpecialKey.DOWN
        KeyEvent.KEYCODE_DPAD_LEFT -> SpecialKey.LEFT
        KeyEvent.KEYCODE_DPAD_RIGHT -> SpecialKey.RIGHT
        KeyEvent.KEYCODE_MOVE_HOME -> SpecialKey.HOME
        KeyEvent.KEYCODE_MOVE_END -> SpecialKey.END
        KeyEvent.KEYCODE_PAGE_UP -> SpecialKey.PAGE_UP
        KeyEvent.KEYCODE_PAGE_DOWN -> SpecialKey.PAGE_DOWN
        KeyEvent.KEYCODE_INSERT -> SpecialKey.INSERT
        KeyEvent.KEYCODE_F1 -> SpecialKey.F1
        KeyEvent.KEYCODE_F2 -> SpecialKey.F2
        KeyEvent.KEYCODE_F3 -> SpecialKey.F3
        KeyEvent.KEYCODE_F4 -> SpecialKey.F4
        KeyEvent.KEYCODE_F5 -> SpecialKey.F5
        KeyEvent.KEYCODE_F6 -> SpecialKey.F6
        KeyEvent.KEYCODE_F7 -> SpecialKey.F7
        KeyEvent.KEYCODE_F8 -> SpecialKey.F8
        KeyEvent.KEYCODE_F9 -> SpecialKey.F9
        KeyEvent.KEYCODE_F10 -> SpecialKey.F10
        KeyEvent.KEYCODE_F11 -> SpecialKey.F11
        KeyEvent.KEYCODE_F12 -> SpecialKey.F12
        else -> null
    }

    private companion object {
        val NO_MODS = KeyMods(ctrl = false, alt = false, shift = false)
    }
}
