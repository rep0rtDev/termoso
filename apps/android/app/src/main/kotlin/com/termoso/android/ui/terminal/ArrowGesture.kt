package com.termoso.android.ui.terminal

import com.termoso.core.SpecialKey
import kotlin.math.abs

/** Keys to send for one step of an arrow gesture. */
data class ArrowStep(val key: SpecialKey, val count: Int)

/**
 * Long-press-and-drag arrows, the Termius way: the finger's displacement from
 * the press point picks the direction (dominant axis), every cell travelled
 * sends one arrow key, and holding still past the dead zone keeps repeating
 * in that direction. Three gears: the farther from the origin, the more keys
 * each repeat tick sends. [speed] scales how far a finger travels per key
 * (1 = one terminal cell, >1 = faster).
 */
class ArrowGesture(
    private val cellWidth: Float,
    private val cellHeight: Float,
    private val speed: Float = 1f,
) {
    private var totalX = 0f
    private var totalY = 0f
    private var pendingX = 0f
    private var pendingY = 0f

    /** Whether the finger left the dead zone at all (a still long-press is not a gesture). */
    val moved: Boolean
        get() = abs(totalX) > cellWidth / 2 || abs(totalY) > cellHeight / 2

    /** Direction of the current displacement, `null` inside the dead zone. */
    val direction: SpecialKey?
        get() = when {
            !moved -> null
            abs(totalX) >= abs(totalY) -> if (totalX > 0) SpecialKey.RIGHT else SpecialKey.LEFT
            else -> if (totalY > 0) SpecialKey.DOWN else SpecialKey.UP
        }

    /** 1..3 — grows with the distance from the press point along the active axis. */
    val gear: Int
        get() {
            val cells = if (abs(totalX) >= abs(totalY)) abs(totalX) / cellWidth else abs(totalY) / cellHeight
            return when {
                cells < GEAR2_CELLS -> 1
                cells < GEAR3_CELLS -> 2
                else -> 3
            }
        }

    /** Feed a finger movement; returns the keys to send for the cells travelled, if any. */
    fun move(dx: Float, dy: Float): ArrowStep? {
        totalX += dx
        totalY += dy
        pendingX += dx
        pendingY += dy
        val dir = direction ?: return null
        return if (dir == SpecialKey.LEFT || dir == SpecialKey.RIGHT) {
            pendingY = 0f
            val cells = (pendingX * speed / cellWidth).toInt()
            if (cells == 0) return null
            pendingX -= cells * cellWidth / speed
            ArrowStep(if (cells > 0) SpecialKey.RIGHT else SpecialKey.LEFT, abs(cells))
        } else {
            pendingX = 0f
            val cells = (pendingY * speed / cellHeight).toInt()
            if (cells == 0) return null
            pendingY -= cells * cellHeight / speed
            ArrowStep(if (cells > 0) SpecialKey.DOWN else SpecialKey.UP, abs(cells))
        }
    }

    /** Finger held still: one tick of auto-repeat in the current direction. */
    fun repeatTick(): ArrowStep? = direction?.let { ArrowStep(it, gear) }

    companion object {
        /** Milliseconds between auto-repeat ticks while the finger is held. */
        const val REPEAT_MS = 140L
        private const val GEAR2_CELLS = 5f
        private const val GEAR3_CELLS = 10f

        /** Settings value (`slow` / `normal` / `fast`) to a travel multiplier. */
        fun speedOf(setting: String): Float = when (setting) {
            "slow" -> 0.6f
            "fast" -> 1.7f
            else -> 1f
        }
    }
}
