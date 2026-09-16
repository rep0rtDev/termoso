package com.termoso.android.ui.terminal

import com.termoso.android.data.TerminalSession
import com.termoso.android.data.userMessage
import com.termoso.core.AiStatusCard
import com.termoso.core.AiTarget
import com.termoso.core.MobileException

/** Server-side limit on the request text (`TERMOSO_AI__MAX_PROMPT_CHARS` default). */
const val AI_MAX_PROMPT_CHARS = 500

/** What the user sees for a failed request and whether trying again makes sense. */
data class AiFailure(val text: String, val retry: Boolean)

/** Stable core error kinds → the words the user needs, same wording as the desktop. */
fun aiFailure(e: Throwable): AiFailure {
    val kind = (e as? MobileException.Other)?.kind
    return when (kind) {
        "ai_not_enabled" -> AiFailure("AI suggestions are turned off for this account.", retry = false)
        "ai_quota_exceeded" -> AiFailure("Today's quota is used up. It resets at midnight UTC.", retry = false)
        "ai_busy" -> AiFailure("The AI provider is busy. Try again in a moment.", retry = true)
        "ai_unavailable" -> AiFailure("The AI provider did not answer. Nothing was inserted.", retry = true)
        "unauthorized" -> AiFailure("Sign in again to use AI suggestions.", retry = false)
        else -> AiFailure(e.userMessage(), retry = true)
    }
}

/** `Chutes · GLM-4.7-Flash`; falls back to a neutral label when the server names nothing. */
fun aiProviderLabel(s: AiStatusCard): String =
    listOfNotNull(s.provider, s.model).joinToString(" · ").ifEmpty { "AI provider" }

/** Today's remaining requests, never negative. */
fun aiRemainingToday(s: AiStatusCard): Int =
    if (s.usedToday >= s.dailyQuota) 0 else (s.dailyQuota - s.usedToday).toInt()

/**
 * Where a request is aimed. A saved host contributes its saved OS label, a
 * local shell says "this phone", anything else (quick connect, a viewer of
 * somebody's share) sends no OS label at all.
 */
fun aiTargetFor(session: TerminalSession): AiTarget = when {
    session.isView -> AiTarget.Unknown
    session.local != null -> AiTarget.Local
    session.hostId != null -> AiTarget.Host(session.hostId)
    else -> AiTarget.Unknown
}

/** What the disclosure line says leaves the phone for this target. */
fun aiContextLabel(target: AiTarget): String = when (target) {
    is AiTarget.Host -> "request · host OS"
    AiTarget.Local -> "request · this phone's OS"
    AiTarget.Unknown -> "request only"
}
