package com.termoso.android.ui.terminal

import com.termoso.core.SpecialKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ArrowGestureTest {
    private fun gesture(speed: Float = 1f) = ArrowGesture(cellWidth = 20f, cellHeight = 40f, speed = speed)

    @Test
    fun still_finger_is_not_a_gesture() {
        val g = gesture()
        assertFalse(g.moved)
        assertNull(g.direction)
        assertNull(g.repeatTick())
        assertNull(g.move(3f, 2f))
        assertFalse(g.moved)
    }

    @Test
    fun one_key_per_cell_along_the_dominant_axis() {
        val g = gesture()
        assertEquals(ArrowStep(SpecialKey.RIGHT, 3), g.move(65f, 4f))
        assertNull(g.move(10f, 0f))
        assertEquals(ArrowStep(SpecialKey.RIGHT, 1), g.move(5f, 0f))
    }

    @Test
    fun vertical_drag_sends_up_and_down() {
        assertEquals(ArrowStep(SpecialKey.UP, 2), gesture().move(0f, -80f))
        assertEquals(ArrowStep(SpecialKey.DOWN, 1), gesture().move(5f, 45f))
    }

    @Test
    fun coming_back_sends_the_opposite_arrow() {
        val g = gesture()
        assertEquals(ArrowStep(SpecialKey.RIGHT, 2), g.move(40f, 0f))
        assertEquals(ArrowStep(SpecialKey.LEFT, 1), g.move(-25f, 0f))
    }

    @Test
    fun gears_grow_with_distance_and_drive_the_repeat() {
        val g = gesture()
        g.move(40f, 0f)
        assertEquals(1, g.gear)
        assertEquals(ArrowStep(SpecialKey.RIGHT, 1), g.repeatTick())
        g.move(80f, 0f)
        assertEquals(2, g.gear)
        assertEquals(ArrowStep(SpecialKey.RIGHT, 2), g.repeatTick())
        g.move(120f, 0f)
        assertEquals(3, g.gear)
        assertEquals(ArrowStep(SpecialKey.RIGHT, 3), g.repeatTick())
    }

    @Test
    fun speed_scales_travel_per_key() {
        assertEquals(ArrowStep(SpecialKey.RIGHT, 1), gesture(speed = 0.5f).move(40f, 0f))
        assertEquals(ArrowStep(SpecialKey.RIGHT, 4), gesture(speed = 2f).move(40f, 0f))
        assertEquals(1f, ArrowGesture.speedOf("normal"))
        assertTrue(ArrowGesture.speedOf("slow") < 1f && ArrowGesture.speedOf("fast") > 1f)
    }
}
