package com.termoso.android.ui.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.termoso.android.R
import com.termoso.core.SpecialKey

/** Live state of a long-press arrow gesture, for the on-screen hint. */
data class ArrowHint(val direction: SpecialKey?, val gear: Int)

/**
 * Small pad shown over the terminal while a long-press arrow gesture is active:
 * four arrows with the current direction lit and one dot per speed gear.
 */
@Composable
fun ArrowGestureOverlay(hint: ArrowHint, modifier: Modifier = Modifier) {
    val label = stringResource(R.string.arrow_gesture)
    Column(
        modifier
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.92f))
            .padding(10.dp)
            .semantics { contentDescription = label },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Arrow(Icons.Filled.KeyboardArrowUp, hint.direction == SpecialKey.UP)
        Row(verticalAlignment = Alignment.CenterVertically) {
            Arrow(Icons.Filled.KeyboardArrowLeft, hint.direction == SpecialKey.LEFT)
            Spacer(Modifier.size(28.dp))
            Arrow(Icons.Filled.KeyboardArrowRight, hint.direction == SpecialKey.RIGHT)
        }
        Arrow(Icons.Filled.KeyboardArrowDown, hint.direction == SpecialKey.DOWN)
        Spacer(Modifier.height(6.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            repeat(3) { i ->
                val on = i < hint.gear
                Box(
                    Modifier
                        .size(6.dp)
                        .clip(CircleShape)
                        .background(
                            if (on) MaterialTheme.colorScheme.primary
                            else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.25f),
                        ),
                )
            }
        }
    }
}

@Composable
private fun Arrow(icon: ImageVector, active: Boolean) {
    Icon(
        icon,
        contentDescription = null,
        modifier = Modifier.size(28.dp),
        tint = if (active) MaterialTheme.colorScheme.primary
        else MaterialTheme.colorScheme.onSurface.copy(alpha = 0.45f),
    )
}
