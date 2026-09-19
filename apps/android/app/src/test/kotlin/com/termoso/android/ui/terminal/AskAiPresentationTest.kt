package com.termoso.android.ui.terminal

import com.termoso.android.ResourceTest
import com.termoso.android.data.usedAfter
import com.termoso.core.AiStatusCard
import com.termoso.core.AiTarget
import com.termoso.core.MobileException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AskAiPresentationTest : ResourceTest() {
    private fun status(
        provider: String? = "Chutes",
        model: String? = "GLM-4.7-Flash",
        quota: UInt = 50u,
        used: UInt = 0u,
    ) = AiStatusCard(
        available = true,
        enabled = true,
        provider = provider,
        model = model,
        confidential = true,
        dailyQuota = quota,
        usedToday = used,
    )

    @Test
    fun stableAiErrorsGetTheirOwnWordsAndRetryHint() {
        val off = aiFailure(MobileException.Other("ai_not_enabled", "server 403: ai_not_enabled: off"))
        assertTrue(off.text.contains("turned off"))
        assertFalse(off.retry)

        val quota = aiFailure(MobileException.Other("ai_quota_exceeded", "server 429: ai_quota_exceeded: quota"))
        assertTrue(quota.text.contains("quota"))
        assertFalse(quota.retry)

        assertTrue(aiFailure(MobileException.Other("ai_busy", "busy")).retry)
        val down = aiFailure(MobileException.Other("ai_unavailable", "down"))
        assertTrue(down.retry)
        assertTrue(down.text.contains("Nothing was inserted"))

        assertFalse(aiFailure(MobileException.Other("unauthorized", "401")).retry)
    }

    @Test
    fun otherFailuresFallBackToTheGenericMessageAndAllowRetry() {
        val f = aiFailure(MobileException.Other("network", "connect: timed out"))
        assertTrue(f.retry)
        assertTrue(f.text.contains("Could not reach the server"))
    }

    @Test
    fun providerLabelJoinsProviderAndModelAndFallsBack() {
        assertEquals("Chutes · GLM-4.7-Flash", aiProviderLabel(status()))
        assertEquals("Chutes", aiProviderLabel(status(model = null)))
        assertEquals("AI provider", aiProviderLabel(status(provider = null, model = null)))
    }

    @Test
    fun remainingQuotaNeverGoesNegative() {
        assertEquals(47, aiRemainingToday(status(used = 3u)))
        assertEquals(0, aiRemainingToday(status(used = 50u)))
        assertEquals(0, aiRemainingToday(status(used = 51u)))
    }

    @Test
    fun usedTodayFollowsTheReplyWithoutUnderflow() {
        assertEquals(4u, usedAfter(50u, 46u))
        assertEquals(0u, usedAfter(50u, 50u))
        assertEquals(0u, usedAfter(50u, 60u))
    }

    @Test
    fun contextLabelNeverNamesTheHost() {
        assertEquals("request · host OS", aiContextLabel(AiTarget.Host("0b6c9f8e-1111-4222-8333-444455556666")))
        assertEquals("request · this phone's OS", aiContextLabel(AiTarget.Local))
        assertEquals("request only", aiContextLabel(AiTarget.Unknown))
    }

    @Test
    fun promptLimitMatchesTheServerDefault() {
        assertEquals(500, AI_MAX_PROMPT_CHARS)
    }
}
