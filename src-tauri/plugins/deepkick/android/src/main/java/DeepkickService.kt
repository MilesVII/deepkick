package com.milesseventh.deepkick.bridge

import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * Keeps the process alive (and shows the progress notification) while the Rust worker
 * thread walks the queue. All the actual work happens in Rust; this service only holds
 * the foreground state.
 */
class DeepkickService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val notification = Notifications.progress(
            this,
            intent?.getStringExtra("title") ?: "DeepKick Lyrics",
            intent?.getStringExtra("text") ?: "",
            intent?.getIntExtra("max", 0) ?: 0,
            intent?.getIntExtra("progress", 0) ?: 0
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                Notifications.ID_PROGRESS,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            )
        } else {
            startForeground(Notifications.ID_PROGRESS, notification)
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            stopForeground(STOP_FOREGROUND_REMOVE)
        } else {
            @Suppress("DEPRECATION")
            stopForeground(true)
        }
        super.onDestroy()
    }
}
