package com.termoso.android.ui.hosts

import com.termoso.core.SnippetItem
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CodingAgentsTest {
    private fun snippet(id: String, script: String) = SnippetItem(
        id = id, vaultId = "v", label = id, script = script, packageId = null,
        closeAfterRun = false, variables = emptyList(), targetHostIds = emptyList(), updatedAt = 0L,
    )

    @Test
    fun matches_a_startup_snippet_by_its_command() {
        val list = listOf(snippet("a", "claude\n"), snippet("b", "ls -la"))
        assertEquals("claude", CodingAgents.selected(list, "a")?.id)
        assertNull(CodingAgents.selected(list, "b"))
        assertNull(CodingAgents.selected(list, null))
    }

    @Test
    fun ids_are_unique_and_resolve() {
        assertEquals(CodingAgents.presets.size, CodingAgents.presets.map { it.id }.distinct().size)
        assertEquals("Gemini CLI", CodingAgents.byId("gemini")?.name)
        assertNull(CodingAgents.byId("nope"))
    }
}
