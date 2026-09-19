package com.termoso.android

import android.app.LocaleManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import java.util.Locale

/**
 * The UI language chosen in Settings. Android 13+ keeps it in the system per-app locale setting
 * (so it also shows up under system Settings → Languages); older releases keep the BCP-47 tag in
 * plain preferences and apply it by wrapping every activity context.
 */
object AppLanguage {
    /** Follow the device language. */
    const val SYSTEM = ""
    val supported = listOf("en", "ru")

    private const val PREFS = "app_language"
    private const val KEY = "tag"

    fun current(context: Context): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val locales = context.getSystemService(LocaleManager::class.java).applicationLocales
            return if (locales.isEmpty) SYSTEM else locales[0].toLanguageTag()
        }
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY, SYSTEM) ?: SYSTEM
    }

    /** Returns true when the caller has to recreate its activities for the change to show. */
    fun set(context: Context, tag: String): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.getSystemService(LocaleManager::class.java).applicationLocales =
                if (tag.isEmpty()) LocaleList.getEmptyLocaleList() else LocaleList.forLanguageTags(tag)
            return false
        }
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(KEY, tag).apply()
        return true
    }

    fun wrap(base: Context): Context {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) return base
        val tag = current(base)
        if (tag.isEmpty()) return base
        val config = Configuration(base.resources.configuration)
        config.setLocales(LocaleList(Locale.forLanguageTag(tag)))
        return base.createConfigurationContext(config)
    }
}
