package com.termoso.android.ui.terminal

import com.termoso.android.R
import androidx.annotation.StringRes
import android.view.KeyEvent

/** App-level shortcut on a physical keyboard, resolved from a key press. */
enum class Hotkey(@StringRes val toast: Int?) {
    PREV_SESSION(null),
    NEXT_SESSION(null),
    CLOSE_SESSION(R.string.session_closed),
    NEW_SESSION(null),
    CLONE_SESSION(R.string.cloning_connection),
    FONT_UP(null),
    FONT_DOWN(null),
    FONT_RESET(null),
    PASTE(null),
    TOGGLE_PANEL(null),
}

/**
 * Pure decision table for keys that never reach the terminal: the volume
 * buttons and the modifier-based app hotkeys. Takes primitives rather than a
 * [KeyEvent] so it can be unit-tested on the JVM.
 */
object HardwareKeys {
    /**
     * Binding for a volume press, or `null` when the key should keep its
     * system meaning (unbound, or a non-volume key).
     */
    fun volumeAction(keyCode: Int, upBinding: String, downBinding: String): InputAction? {
        val raw = when (keyCode) {
            KeyEvent.KEYCODE_VOLUME_UP -> upBinding
            KeyEvent.KEYCODE_VOLUME_DOWN -> downBinding
            else -> return null
        }
        return InputAction.parse(raw).takeIf { it != InputAction.Disabled }
    }

    /**
     * Hotkey for a Ctrl(+Shift) chord under the `hardware_hotkeys` mode
     * (`disabled` | `ctrl` | `ctrl_shift`). Ctrl+Shift+V (paste) and
     * Ctrl+Shift+K (key panel) work in both enabled modes, matching desktop
     * terminals; everything else follows the mode. `disabled` lets every
     * chord through to the shell.
     */
    fun hotkey(keyCode: Int, ctrl: Boolean, shift: Boolean, alt: Boolean, mode: String): Hotkey? {
        if (!ctrl || alt || mode == "disabled") return null
        if (shift) {
            when (keyCode) {
                KeyEvent.KEYCODE_V -> return Hotkey.PASTE
                KeyEvent.KEYCODE_K -> return Hotkey.TOGGLE_PANEL
            }
        }
        val armed = when (mode) {
            "ctrl" -> !shift || keyCode == KeyEvent.KEYCODE_EQUALS || keyCode == KeyEvent.KEYCODE_PLUS
            "ctrl_shift" -> shift
            else -> false
        }
        if (!armed) return null
        return when (keyCode) {
            KeyEvent.KEYCODE_DPAD_LEFT -> Hotkey.PREV_SESSION
            KeyEvent.KEYCODE_DPAD_RIGHT -> Hotkey.NEXT_SESSION
            KeyEvent.KEYCODE_W -> Hotkey.CLOSE_SESSION
            KeyEvent.KEYCODE_T -> Hotkey.NEW_SESSION
            KeyEvent.KEYCODE_N -> Hotkey.CLONE_SESSION
            KeyEvent.KEYCODE_EQUALS, KeyEvent.KEYCODE_PLUS, KeyEvent.KEYCODE_NUMPAD_ADD -> Hotkey.FONT_UP
            KeyEvent.KEYCODE_MINUS, KeyEvent.KEYCODE_NUMPAD_SUBTRACT -> Hotkey.FONT_DOWN
            KeyEvent.KEYCODE_0, KeyEvent.KEYCODE_NUMPAD_0 -> Hotkey.FONT_RESET
            else -> null
        }
    }

    /** Index of the session to show after a left/right switch; wraps around. */
    fun neighbour(count: Int, current: Int, forward: Boolean): Int? {
        if (count < 2 || current !in 0 until count) return null
        return if (forward) (current + 1) % count else (current - 1 + count) % count
    }

    val hotkeyModes = listOf(
        "disabled" to R.string.disabled,
        "ctrl" to R.string.ctrl,
        "ctrl_shift" to R.string.ctrl_shift,
    )
}
