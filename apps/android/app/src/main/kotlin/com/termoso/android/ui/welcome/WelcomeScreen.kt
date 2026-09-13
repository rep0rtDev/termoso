package com.termoso.android.ui.welcome

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Cloud
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Key
import androidx.compose.material.icons.filled.Terminal
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.termoso.android.ui.theme.DarkLayers
import com.termoso.android.ui.theme.Emerald

/**
 * First-launch screen. Cloud sign-in / sign-up land with the account phase;
 * until then both lead to [onCloud] so the caller can explain.
 */
@Composable
fun WelcomeScreen(onCloud: () -> Unit, onContinueOffline: () -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .background(DarkLayers.lowest)
            .systemBarsPadding(),
    ) {
        Column(
            Modifier
                .fillMaxSize()
                .padding(horizontal = 28.dp, vertical = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(Modifier.weight(1f))
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Tile(Icons.Filled.Terminal)
                Tile(Icons.Filled.Key)
            }
            Spacer(Modifier.height(14.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Tile(Icons.Filled.Cloud)
                Tile(Icons.Filled.Group)
            }
            Spacer(Modifier.height(36.dp))
            Text(
                "Welcome to Termoso",
                style = MaterialTheme.typography.headlineMedium,
                fontWeight = FontWeight.Bold,
                color = DarkLayers.text,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                "Free, open-source SSH client. End-to-end encrypted sync, teams and live sharing — no telemetry.",
                style = MaterialTheme.typography.bodyMedium,
                color = DarkLayers.secondary,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(32.dp))
            Button(
                onClick = onCloud,
                modifier = Modifier.fillMaxWidth().height(48.dp),
                shape = RoundedCornerShape(12.dp),
            ) { Text("Create a free account") }
            Spacer(Modifier.height(10.dp))
            OutlinedButton(
                onClick = onCloud,
                modifier = Modifier.fillMaxWidth().height(48.dp),
                shape = RoundedCornerShape(12.dp),
            ) { Text("Sign in", color = DarkLayers.text) }
            Spacer(Modifier.height(12.dp))
            Text(
                "Termoso Cloud is completely free for everyone — no limits, no plans, no strings attached.",
                style = MaterialTheme.typography.bodySmall,
                color = DarkLayers.secondary,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.weight(1f))
            TextButton(onClick = onContinueOffline) {
                Text("Continue without sync", color = DarkLayers.text)
            }
            Text(
                "Free forever · Open source · No telemetry",
                style = MaterialTheme.typography.labelMedium,
                color = DarkLayers.secondary,
            )
        }
    }
}

@Composable
private fun Tile(icon: ImageVector) {
    Box(
        Modifier
            .size(56.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(Emerald),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = null, tint = Color.White, modifier = Modifier.size(28.dp))
    }
}
