package com.termoso.android.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val DarkScheme = darkColorScheme(
    primary = Emerald,
    onPrimary = Color.White,
    primaryContainer = EmeraldDark,
    onPrimaryContainer = Color.White,
    secondary = Sky,
    onSecondary = Color.White,
    background = DarkLayers.base,
    onBackground = DarkLayers.text,
    surface = DarkLayers.base,
    onSurface = DarkLayers.text,
    surfaceVariant = DarkLayers.high,
    onSurfaceVariant = DarkLayers.secondary,
    surfaceContainerLowest = DarkLayers.lowest,
    surfaceContainerLow = DarkLayers.base,
    surfaceContainer = DarkLayers.high,
    surfaceContainerHigh = DarkLayers.highest,
    surfaceContainerHighest = DarkLayers.strong,
    outline = DarkLayers.strong,
    outlineVariant = DarkLayers.high,
    error = Danger,
    onError = Color.White,
)

private val LightScheme = lightColorScheme(
    primary = EmeraldDark,
    onPrimary = Color.White,
    primaryContainer = EmeraldLight,
    onPrimaryContainer = LightLayers.text,
    secondary = Sky,
    onSecondary = Color.White,
    background = LightLayers.base,
    onBackground = LightLayers.text,
    surface = LightLayers.base,
    onSurface = LightLayers.text,
    surfaceVariant = LightLayers.highest,
    onSurfaceVariant = LightLayers.secondary,
    surfaceContainerLowest = LightLayers.lowest,
    surfaceContainerLow = LightLayers.base,
    surfaceContainer = LightLayers.high,
    surfaceContainerHigh = LightLayers.highest,
    surfaceContainerHighest = LightLayers.strong,
    outline = LightLayers.strong,
    outlineVariant = LightLayers.highest,
    error = Danger,
    onError = Color.White,
)

/** `system` / `dark` / `light` — same values as `MobileSettings.appTheme`. */
@Composable
fun TermosoTheme(appTheme: String = "system", content: @Composable () -> Unit) {
    val dark = when (appTheme) {
        "dark" -> true
        "light" -> false
        else -> isSystemInDarkTheme()
    }
    val scheme = if (dark) DarkScheme else LightScheme
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !dark
                isAppearanceLightNavigationBars = !dark
            }
        }
    }
    MaterialTheme(colorScheme = scheme, typography = TermosoTypography, content = content)
}
