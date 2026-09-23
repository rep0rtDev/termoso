package com.termoso.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LinksTest {
    private val id = "0f0e0d0c-0b0a-4908-8706-050403020100"
    private val secret = "A".repeat(43)

    @Test
    fun inviteLinks() {
        assertEquals(LinkKind.Invite, classifyLink("termoso://invite/tok_abc"))
        assertEquals(LinkKind.Invite, classifyLink("https://app.termoso.com/invite/tok_abc"))
        assertEquals(LinkKind.Invite, classifyLink(" https://host.example:8443/termoso/invite/tok_abc?utm=x "))
        assertEquals(LinkKind.Other, classifyLink("termoso://invite/"))
        assertEquals(LinkKind.Other, classifyLink("termoso://invite/a/b"))
        assertEquals(LinkKind.Other, classifyLink("https://app.termoso.com/invite"))
        assertEquals(LinkKind.Other, classifyLink("http://app.termoso.com/invite/tok_abc"))
    }

    @Test
    fun joinLinks() {
        assertEquals(LinkKind.Join, classifyLink("termoso://join/$id?s=https%3A%2F%2Fapi.test%2F#$secret"))
        assertEquals(LinkKind.Join, classifyLink("termoso://join/$id"))
        assertEquals(LinkKind.Join, classifyLink("https://app.termoso.com/join/$id#$secret"))
        assertEquals(LinkKind.Join, classifyLink("https://host.example/termoso/join/$id/#$secret"))
        assertEquals(LinkKind.Other, classifyLink("termoso://join/$id/extra#$secret"))
        assertEquals(LinkKind.Other, classifyLink("https://app.termoso.com/joined/$id#$secret"))
    }

    @Test
    fun otherLinks() {
        assertEquals(LinkKind.Other, classifyLink("ssh://root@host"))
        assertEquals(LinkKind.Other, classifyLink("https://app.termoso.com/"))
        assertEquals(LinkKind.Other, classifyLink("https://app.termoso.com/login?next=%2Finvite%2Ftok"))
        assertEquals(LinkKind.Other, classifyLink("termoso://vault/x"))
        assertEquals(LinkKind.Other, classifyLink("not a link"))
        assertEquals(LinkKind.Other, classifyLink("https://[bad/invite/x"))
        assertEquals(LinkKind.Other, classifyLink("termoso://sso?flow=${"a".repeat(24)}"))
    }

    @Test
    fun ssoCallback() {
        val flow = "Ab0_-" + "z".repeat(19)
        assertEquals(flow, parseSsoLink("termoso://sso?flow=$flow"))
        assertEquals(flow, parseSsoLink(" TERMOSO://SSO/?flow=$flow "))
        assertEquals("a".repeat(16), parseSsoLink("termoso://sso?flow=${"a".repeat(16)}"))
        assertEquals("a".repeat(128), parseSsoLink("termoso://sso?flow=${"a".repeat(128)}"))
    }

    @Test
    fun ssoCallbackRejectsAnythingElse() {
        val flow = "a".repeat(24)
        listOf(
            "termoso://sso",
            "termoso://sso?",
            "termoso://sso?flow=",
            "termoso://sso?flow",
            "termoso://sso?flow=${"a".repeat(15)}",
            "termoso://sso?flow=${"a".repeat(129)}",
            "termoso://sso?flow=$flow&flow=$flow",
            "termoso://sso?flow=$flow&access_token=x",
            "termoso://sso?flow=$flow&sso_session=x",
            "termoso://sso?sso_session=$flow",
            "termoso://sso?flow=$flow#token",
            "termoso://sso/callback?flow=$flow",
            "termoso://sso:1?flow=$flow",
            "termoso://user@sso?flow=$flow",
            "termoso://ssox?flow=$flow",
            "termoso://invite/$flow",
            "https://sso?flow=$flow",
            "termoso://sso?flow=a b",
            "termoso://sso?flow=${flow}%2F",
            "termoso://sso?flow=${flow}/",
            "termoso://sso?flow=$flow&",
            "termoso://[bad?flow=$flow",
        ).forEach { assertNull(it, parseSsoLink(it)) }
    }
}
