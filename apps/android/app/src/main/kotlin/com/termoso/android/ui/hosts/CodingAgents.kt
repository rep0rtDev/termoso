package com.termoso.android.ui.hosts

import com.termoso.core.SnippetItem

/** A CLI coding agent that can be started automatically when a host connects. */
data class CodingAgent(val id: String, val name: String, val command: String)

/**
 * Presets for the "AI coding agent" picker in the host editor. Picking one
 * creates (or reuses) a startup snippet with the agent's launch command, so
 * the agent is running by the time the terminal is on screen — Termius'
 * "configure Claude Code / Gemini on your hosts" flow, built on snippets.
 */
object CodingAgents {
    val presets = listOf(
        CodingAgent("claude", "Claude Code", "claude"),
        CodingAgent("gemini", "Gemini CLI", "gemini"),
        CodingAgent("codex", "Codex CLI", "codex"),
        CodingAgent("opencode", "OpenCode", "opencode"),
        CodingAgent("aider", "Aider", "aider"),
    )

    fun byId(id: String?): CodingAgent? = presets.firstOrNull { it.id == id }

    /** The preset a snippet launches, if its script is exactly one of the agent commands. */
    fun of(snippet: SnippetItem?): CodingAgent? {
        val script = snippet?.script?.trim() ?: return null
        return presets.firstOrNull { it.command == script }
    }

    /** The preset currently selected for a host via its startup snippet. */
    fun selected(snippets: List<SnippetItem>, startupSnippetId: String?): CodingAgent? =
        of(snippets.firstOrNull { it.id == startupSnippetId })

    fun snippetLabel(agent: CodingAgent): String = "Start ${agent.name}"
}
