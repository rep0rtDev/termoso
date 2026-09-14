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

/**
 * Foreground service that runs only while at least one terminal is open. It
 * holds no session state itself — the sockets live in Rust inside the app
 * process — it just keeps that process out of the background-kill lists and
 * shows the "N sessions" notification Android requires in return.
 */
class SessionService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val count = intent?.getIntExtra(EXTRA_COUNT, 0) ?: 0
        if (count <= 0) {
            stopSelf()
            return START_NOT_STICKY
        }
        ensureChannel()
        val notification = build(count)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        return START_NOT_STICKY
    }

    private fun build(count: Int): Notification {
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val text = if (count == 1) "1 active session" else "$count active sessions"
        return Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Termoso")
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .build()
    }

    private fun ensureChannel() {
        val manager = getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(CHANNEL_ID, "Active sessions", NotificationManager.IMPORTANCE_LOW).apply {
            description = "Shown while a terminal is connected so the connection survives in the background."
            setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
    }

    companion object {
        private const val CHANNEL_ID = "sessions"
        private const val NOTIFICATION_ID = 1
        private const val EXTRA_COUNT = "count"

        /** Start, update or stop the service to match [count] open sessions. */
        fun sync(context: Context, count: Int) {
            val intent = Intent(context, SessionService::class.java).putExtra(EXTRA_COUNT, count)
            if (count <= 0) {
                context.stopService(intent)
                return
            }
            // Only the user connecting (app in front) can start it; later count
            // changes reach a service that is already in the foreground.
            runCatching { context.startForegroundService(intent) }
        }
    }
}
