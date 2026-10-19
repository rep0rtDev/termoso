package com.termoso.android.data

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import com.termoso.android.R

/**
 * Launcher long-press shortcuts for the hosts connected to most recently
 * (`termoso://host/<id>`), next to the static "New connection / Local terminal /
 * SFTP" ones from `res/xml/shortcuts.xml`. Labels only — never addresses with
 * credentials, never anything from a locked vault.
 */
object RecentShortcuts {
    private const val STATIC_COUNT = 3
    private class Action(val id: String, val label: Int, val icon: Int, val link: String)

    private val BASE = listOf(
        Action("action:new", R.string.shortcut_new_connection, R.drawable.ic_shortcut_new, "termoso://new"),
        Action("action:local", R.string.shortcut_local_terminal, R.drawable.ic_shortcut_terminal, "termoso://local"),
        Action("action:sftp", R.string.shortcut_sftp, R.drawable.ic_shortcut_sftp, "termoso://sftp"),
    )

    /**
     * The three fixed actions, published as dynamic shortcuts (a static
     * `shortcuts.xml` cannot follow the `.debug` application id). Idempotent.
     */
    fun ensureBase(context: Context) {
        val base = BASE.mapIndexed { rank, a ->
            ShortcutInfoCompat.Builder(context, a.id)
                .setShortLabel(context.getString(a.label))
                .setLongLabel(context.getString(a.label))
                .setRank(rank)
                .setIcon(IconCompat.createWithResource(context, a.icon))
                .setIntent(Intent(Intent.ACTION_VIEW, Uri.parse(a.link)).setPackage(context.packageName))
                .build()
        }
        runCatching {
            val present = ShortcutManagerCompat.getDynamicShortcuts(context).map { it.id }.toSet()
            val (existing, missing) = base.partition { it.id in present }
            if (missing.isNotEmpty()) ShortcutManagerCompat.addDynamicShortcuts(context, missing)
            // Icons and labels of the published ones follow the app (older builds used the launcher icon).
            if (existing.isNotEmpty()) ShortcutManagerCompat.updateShortcuts(context, existing)
        }
    }

    suspend fun record(context: Context, repo: VaultRepository, hostId: String, vaultId: String) {
        val host = runCatching { repo.read { hosts(vaultId).firstOrNull { it.id == hostId } } }.getOrNull() ?: return
        val shortcut = ShortcutInfoCompat.Builder(context, "host:$hostId")
            .setShortLabel(host.label.ifBlank { host.address })
            .setLongLabel(host.label.ifBlank { host.address })
            .setIcon(IconCompat.createWithResource(context, R.drawable.ic_shortcut_host))
            .setIntent(Intent(Intent.ACTION_VIEW, Uri.parse("termoso://host/$hostId")).setPackage(context.packageName))
            .build()
        runCatching {
            val max = (ShortcutManagerCompat.getMaxShortcutCountPerActivity(context) - STATIC_COUNT).coerceAtLeast(1)
            val current = ShortcutManagerCompat.getDynamicShortcuts(context)
            val base = current.filter { it.id.startsWith("action:") }
            val hosts = current.filter { it.id.startsWith("host:") && it.id != shortcut.id }
            ShortcutManagerCompat.setDynamicShortcuts(context, base + (listOf(shortcut) + hosts).take(max))
            ShortcutManagerCompat.reportShortcutUsed(context, shortcut.id)
        }
    }

    /** Sign-out / vault removal: nothing from the vault may stay in the launcher. */
    fun clear(context: Context) {
        runCatching {
            val hosts = ShortcutManagerCompat.getDynamicShortcuts(context).map { it.id }.filter { it.startsWith("host:") }
            if (hosts.isNotEmpty()) ShortcutManagerCompat.removeDynamicShortcuts(context, hosts)
        }
    }
}
