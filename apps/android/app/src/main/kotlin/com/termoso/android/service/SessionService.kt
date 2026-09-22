package com.termoso.android.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import com.termoso.android.MainActivity
import com.termoso.android.R
import com.termoso.android.str
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/** What is keeping the process alive, by kind; the notification lists each non-zero part. */
data class ActiveCounts(val terminals: Int = 0, val files: Int = 0, val tunnels: Int = 0) {
    val total: Int get() = terminals + files + tunnels
}

/**
 * Foreground service that runs only while at least one terminal, file connection
 * or tunnel is open. It holds no session state itself — the sockets live in Rust
 * inside the app process — it just keeps that process out of the background-kill
 * lists and shows the "N active" notification Android requires in return.
 *
 * Counts travel through [counts] rather than start intents: a start may be refused
 * while the app is in the background, but the running service still sees every
 * change and stops itself the moment nothing is left.
 */
class SessionService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        ensureChannel()
        scope.launch {
            counts.drop(1).collect { apply(it) }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        apply(counts.value)
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun apply(counts: ActiveCounts) {
        if (counts.total <= 0) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return
        }
        val notification = build(counts)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun build(counts: ActiveCounts): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(describe(counts))
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .build()
    }

    private fun describe(counts: ActiveCounts): String {
        val res = resources
        return buildList {
            if (counts.terminals > 0) add(res.getQuantityString(R.plurals.notif_terminals, counts.terminals, counts.terminals))
            if (counts.files > 0) add(res.getQuantityString(R.plurals.notif_file_connections, counts.files, counts.files))
            if (counts.tunnels > 0) add(res.getQuantityString(R.plurals.notif_tunnels, counts.tunnels, counts.tunnels))
        }.joinToString(" · ")
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
        private const val NOTIFICATION_ID = 1

        private val _counts = MutableStateFlow(ActiveCounts())
        val counts: StateFlow<ActiveCounts> = _counts.asStateFlow()

        /** Start, update or stop the service to match what is open. */
        fun sync(context: Context, counts: ActiveCounts) {
            _counts.value = counts
            val intent = Intent(context, SessionService::class.java)
            if (counts.total <= 0) {
                context.stopService(intent)
                return
            }
            // Only the user connecting (app in front) can start it; later changes
            // reach the running service through [counts].
            runCatching { context.startForegroundService(intent) }
        }

        /** Retry a start that was refused while the app was in the background. */
        fun ensureRunning(context: Context) {
            if (_counts.value.total <= 0) return
            runCatching { context.startForegroundService(Intent(context, SessionService::class.java)) }
        }
    }
}
