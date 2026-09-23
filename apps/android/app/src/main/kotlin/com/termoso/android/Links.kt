package com.termoso.android

import java.net.URI
import java.net.URISyntaxException

/** What an incoming `ACTION_VIEW` link asks the app to do. */
enum class LinkKind { Invite, Join, Other }

/**
 * Classifies deep links the manifest routes to [MainActivity]:
 *
 * - `termoso://invite/<token>` and `https://<server>/invite/<token>` (team invitations);
 * - `termoso://join/<session>?s=<server>#<secret>` and `https://<server>/join/<session>#<secret>`
 *   (live terminal sharing).
 *
 * The https forms are the links people actually share; Android hands them to the app directly
 * once App Links are verified for the host, otherwise the web landing page offers the `termoso://`
 * form. Only the shape is checked here — tokens, sessions and the join secret (carried in the
 * fragment, which never reaches any server) are validated in Rust / by the server, so a truncated
 * link gets a real error instead of being dropped silently.
 */
fun classifyLink(link: String): LinkKind {
    val uri = try {
        URI(link.trim())
    } catch (_: URISyntaxException) {
        return LinkKind.Other
    }
    val segments = uri.rawPath.orEmpty().split('/').filter { it.isNotEmpty() }
    return when (uri.scheme?.lowercase()) {
        "termoso" -> when {
            segments.size != 1 -> LinkKind.Other
            uri.host == "invite" -> LinkKind.Invite
            uri.host == "join" -> LinkKind.Join
            else -> LinkKind.Other
        }
        "https" -> when {
            uri.host.isNullOrEmpty() || segments.size < 2 -> LinkKind.Other
            segments[segments.size - 2] == "invite" -> LinkKind.Invite
            segments[segments.size - 2] == "join" -> LinkKind.Join
            else -> LinkKind.Other
        }
        else -> LinkKind.Other
    }
}

private val SSO_FLOW_ID = Regex("^[A-Za-z0-9_-]{16,128}$")

/**
 * Flow id from a single sign-on callback, or null for anything else.
 *
 * The only accepted shape is `termoso://sso?flow=<id>`: nothing but the flow id may ride along
 * (no path, no fragment, no second parameter), so a provider or a page that tries to hand the app
 * a token or a session through the callback is dropped here before Rust ever sees it. The id
 * itself is opaque — Rust only accepts one that matches the flow this app started.
 */
fun parseSsoLink(link: String): String? {
    val uri = try {
        URI(link.trim())
    } catch (_: URISyntaxException) {
        return null
    }
    if (uri.scheme?.lowercase() != "termoso" || uri.host?.lowercase() != "sso") return null
    if (uri.rawPath.orEmpty().trimEnd('/').isNotEmpty() || uri.rawFragment != null) return null
    if (uri.rawUserInfo != null || uri.port != -1) return null
    val query = uri.rawQuery ?: return null
    val params = query.split('&')
    if (params.size != 1) return null
    val (key, value) = params[0].split('=', limit = 2).let { it[0] to it.getOrNull(1) }
    if (key != "flow" || value == null) return null
    return value.takeIf { SSO_FLOW_ID.matches(it) }
}
