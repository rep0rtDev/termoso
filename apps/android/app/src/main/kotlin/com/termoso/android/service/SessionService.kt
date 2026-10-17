package com.termoso.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Matrix
import android.graphics.Paint
import android.os.Build
import android.os.IBinder
import androidx.compose.ui.graphics.toArgb
import androidx.core.graphics.PathParser
import com.termoso.android.MainActivity
import com.termoso.android.R
import com.termoso.android.str
import com.termoso.android.ui.components.DistroIcons
import com.termoso.android.ui.components.SvgPath
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/** One live connection as the shade shows it. */
data class ActiveItem(val id: String, val title: String, val text: String, val osName: String? = null)

/** What is keeping the process alive; the shade lists every item and sums them up. */
data class ActiveItems(
    val terminals: List<ActiveItem> = emptyList(),
    val files: List<ActiveItem> = emptyList(),
    val tunnels: List<ActiveItem> = emptyList(),
) {
    val total: Int get() = terminals.size + files.size + tunnels.size
}

/** How the notification's buttons reach the managers (same process, set by KeepAlive). */
interface SessionControls {
    fun closeTerminal(id: String)
    fun closeFile(id: String)
    fun stopTunnel(ruleId: String)
    fun closeAll()
}

/**
 * Foreground service that runs only while at least one terminal, file connection
 * or tunnel is open. It holds no session state itself — the sockets live in Rust
 * inside the app process — it just keeps that process out of the background-kill
 * lists and shows the notifications Android requires in return: a summary with
 * the counts and "Close all", plus one entry per connection with "Close", the
 * way Termius does it.
 *
 * Items travel through [items] rather than start intents: a start may be refused
 * while the app is in the background, but the running service still sees every
 * change and stops itself the moment nothing is left.
 */
class SessionService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var shown = emptySet<Int>()

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        ensureChannel()
        scope.launch {
            items.drop(1).collect { apply(it) }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val id = intent?.getStringExtra(EXTRA_ID)
        when (intent?.action) {
            ACTION_CLOSE_TERMINAL -> id?.let { controls?.closeTerminal(it) }
            ACTION_CLOSE_FILE -> id?.let { controls?.closeFile(it) }
            ACTION_STOP_TUNNEL -> id?.let { controls?.stopTunnel(it) }
            ACTION_CLOSE_ALL -> controls?.closeAll()
        }
        apply(items.value)
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun apply(items: ActiveItems) {
        val manager = getSystemService(NotificationManager::class.java)
        if (items.total <= 0) {
            shown.forEach(manager::cancel)
            shown = emptySet()
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return
        }
        val summary = buildSummary(items)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, summary, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, summary)
        }
        val now = mutableSetOf<Int>()
        fun post(item: ActiveItem, notification: Notification) {
            val nid = item.id.hashCode().let { if (it == NOTIFICATION_ID) it + 1 else it }
            now += nid
            manager.notify(nid, notification)
        }
        items.terminals.forEach { post(it, buildItem(it, ACTION_OPEN_TERMINAL, ACTION_CLOSE_TERMINAL, R.string.close)) }
        items.files.forEach { post(it, buildItem(it, ACTION_OPEN_FILES, ACTION_CLOSE_FILE, R.string.close)) }
        items.tunnels.forEach { post(it, buildItem(it, ACTION_OPEN_FORWARDING, ACTION_STOP_TUNNEL, R.string.stop)) }
        (shown - now).forEach(manager::cancel)
        shown = now
    }

    private fun openApp(action: String, id: String? = null, request: Int = 0): PendingIntent =
        PendingIntent.getActivity(
            this,
            request,
            Intent(this, MainActivity::class.java)
                .setAction(action)
                .putExtra(EXTRA_ID, id)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun serviceAction(action: String, id: String? = null, request: Int = 0): PendingIntent =
        PendingIntent.getService(
            this,
            request,
            Intent(this, SessionService::class.java).setAction(action).putExtra(EXTRA_ID, id),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun buildSummary(items: ActiveItems): Notification {
        val res = resources
        val lines = buildList {
            if (items.terminals.isNotEmpty()) add(res.getQuantityString(R.plurals.notif_terminals, items.terminals.size, items.terminals.size))
            if (items.tunnels.isNotEmpty()) add(res.getQuantityString(R.plurals.notif_tunnels, items.tunnels.size, items.tunnels.size))
            if (items.files.isNotEmpty()) add(res.getQuantityString(R.plurals.notif_file_connections, items.files.size, items.files.size))
        }
        // Tap lands where the work is: terminals first, then forwarding, then files.
        val open = when {
            items.terminals.isNotEmpty() -> openApp(ACTION_OPEN_CONNECTIONS)
            items.tunnels.isNotEmpty() -> openApp(ACTION_OPEN_FORWARDING)
            else -> openApp(ACTION_OPEN_FILES, items.files.first().id)
        }
        val style = Notification.InboxStyle().setBigContentTitle(getString(R.string.app_name))
        lines.forEach { style.addLine(it) }
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(lines.joinToString(" · "))
            .setStyle(style)
            .setContentIntent(open)
            .addAction(
                Notification.Action.Builder(null, getString(R.string.close_all), serviceAction(ACTION_CLOSE_ALL)).build(),
            )
            .setGroup(GROUP)
            .setGroupSummary(true)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .build()
    }

    private fun buildItem(item: ActiveItem, openAction: String, closeAction: String, closeLabel: Int): Notification {
        val request = item.id.hashCode()
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setLargeIcon(distroBitmap(item.osName))
            .setContentTitle(item.title)
            .setContentText(item.text)
            .setContentIntent(openApp(openAction, item.id, request))
            .addAction(
                Notification.Action.Builder(null, getString(closeLabel), serviceAction(closeAction, item.id, request)).build(),
            )
            .setGroup(GROUP)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .build()
    }

    /** The host's distro mark on a round brand-coloured badge, or nothing if the OS is unknown. */
    private fun distroBitmap(osName: String?): Bitmap? {
        val icon = DistroIcons.forOs(osName) ?: return null
        val size = resources.getDimensionPixelSize(android.R.dimen.notification_large_icon_width)
        val bitmap = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        paint.color = icon.color.toArgb()
        canvas.drawCircle(size / 2f, size / 2f, size / 2f, paint)
        val path = runCatching { PathParser.createPathFromPathData(SvgPath.normalize(icon.path)) }.getOrNull() ?: return bitmap
        val scale = size * 0.6f / 24f
        path.transform(Matrix().apply { setScale(scale, scale); postTranslate(size * 0.2f, size * 0.2f) })
        paint.color = 0xFFFFFFFF.toInt()
        canvas.drawPath(path, paint)
        return bitmap
    }

    private fun ensureChannel() {
        val manager = getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(CHANNEL_ID, str(R.string.active_sessions_2), NotificationManager.IMPORTANCE_LOW).apply {
            description = str(R.string.shown_while_a_terminal_is_connected_so_the)
            setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
    }

    companion object {
        private const val CHANNEL_ID = "sessions"
        private const val GROUP = "com.termoso.android.sessions"
        private const val NOTIFICATION_ID = 1
        const val EXTRA_ID = "com.termoso.android.extra.ID"
        const val ACTION_OPEN_TERMINAL = "com.termoso.android.action.OPEN_TERMINAL"
        const val ACTION_OPEN_CONNECTIONS = "com.termoso.android.action.OPEN_CONNECTIONS"
        const val ACTION_OPEN_FORWARDING = "com.termoso.android.action.OPEN_FORWARDING"
        const val ACTION_OPEN_FILES = "com.termoso.android.action.OPEN_FILES"
        private const val ACTION_CLOSE_TERMINAL = "com.termoso.android.action.CLOSE_TERMINAL"
        private const val ACTION_CLOSE_FILE = "com.termoso.android.action.CLOSE_FILE"
        private const val ACTION_STOP_TUNNEL = "com.termoso.android.action.STOP_TUNNEL"
        private const val ACTION_CLOSE_ALL = "com.termoso.android.action.CLOSE_ALL"

        private val _items = MutableStateFlow(ActiveItems())
        val items: StateFlow<ActiveItems> = _items.asStateFlow()

        /** Where the buttons go; set by the owner of the managers while the vault is open. */
        @Volatile
        var controls: SessionControls? = null

        /** Start, update or stop the service to match what is open. */
        fun sync(context: Context, items: ActiveItems) {
            _items.value = items
            val intent = Intent(context, SessionService::class.java)
            if (items.total <= 0) {
                context.stopService(intent)
                return
            }
            // Only the user connecting (app in front) can start it; later changes
            // reach the running service through [items].
            runCatching { context.startForegroundService(intent) }
        }

        /** Retry a start that was refused while the app was in the background. */
        fun ensureRunning(context: Context) {
            if (_items.value.total <= 0) return
            runCatching { context.startForegroundService(Intent(context, SessionService::class.java)) }
        }
    }
}
