package com.termoso.android

import org.junit.Assert.assertEquals
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
    }
}
