package com.milesseventh.deepkick

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

class LyricsForegroundService : Service() {

    companion object {
        const val CHANNEL_ID = "lyrics_processing"
        const val NOTIFICATION_ID = 1001
        const val ACTION_CANCEL = "com.milesseventh.deepkick.CANCEL"
        const val EXTRA_SESSION_ID = "session_id"
        const val EXTRA_PROGRESS = "progress"
        const val EXTRA_CURRENT_TRACK = "current_track"
        const val EXTRA_TOTAL = "total"
    }

    private var notificationManager: NotificationManager? = null
    private var currentSessionId: String? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val sessionId = intent?.getStringExtra(EXTRA_SESSION_ID) ?: return START_NOT_STICKY
        currentSessionId = sessionId

        val progress = intent.getIntExtra(EXTRA_PROGRESS, 0)
        val currentTrack = intent.getStringExtra(EXTRA_CURRENT_TRACK) ?: ""
        val total = intent.getIntExtra(EXTRA_TOTAL, 0)

        val notification = buildNotification(progress, currentTrack, total, sessionId)
        startForeground(NOTIFICATION_ID, notification)

        return START_STICKY
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Lyrics Processing",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Shows progress of lyrics downloading and embedding"
                setShowBadge(false)
            }
            notificationManager?.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(
        progress: Int,
        currentTrack: String,
        total: Int,
        sessionId: String
    ): Notification {
        val cancelIntent = Intent(this, LyricsForegroundService::class.java).apply {
            action = ACTION_CANCEL
            putExtra(EXTRA_SESSION_ID, sessionId)
        }
        val cancelPendingIntent = PendingIntent.getService(
            this, 0, cancelIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val contentIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val contentPendingIntent = PendingIntent.getActivity(
            this, 0, contentIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val progressText = if (total > 0) {
            "Processing: $currentTrack ($progress/$total)"
        } else {
            "Preparing..."
        }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("DeepKick Lyrics: Processing...")
            .setContentText(progressText)
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setProgress(total, progress, false)
            .setContentIntent(contentPendingIntent)
            .addAction(
                NotificationCompat.Action.Builder(
                    android.R.drawable.ic_menu_close_clear_cancel,
                    "Cancel",
                    cancelPendingIntent
                ).build()
            )
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .build()
    }

    fun updateProgress(progress: Int, currentTrack: String, total: Int) {
        val notification = buildNotification(progress, currentTrack, total, currentSessionId ?: "")
        notificationManager?.notify(NOTIFICATION_ID, notification)
    }

    fun completeProcessing(found: Int, notFound: Int, errors: Int, total: Int) {
        val summaryText = "Complete: $found found, $notFound not found, $errors errors"

        val contentIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            putExtra("show_report", true)
        }
        val contentPendingIntent = PendingIntent.getActivity(
            this, 0, contentIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("DeepKick Lyrics: Processing Complete")
            .setContentText(summaryText)
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(false)
            .setAutoCancel(true)
            .setContentIntent(contentPendingIntent)
            .setCategory(NotificationCompat.CATEGORY_STATUS)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .build()

        notificationManager?.notify(NOTIFICATION_ID, notification)
        stopForeground(true)
        stopSelf()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        stopForeground(true)
    }
}