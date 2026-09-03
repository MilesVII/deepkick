package com.milesseventh.deepkick

import android.app.NotificationManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {

    companion object {
        const val EXTRA_SHOW_REPORT = "show_report"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        // Handle notification click to show report
        if (intent?.getBooleanExtra(EXTRA_SHOW_REPORT, false) == true) {
            // The webview will handle showing the report via Tauri event
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (intent.getBooleanExtra(EXTRA_SHOW_REPORT, false) == true) {
            // Emit event to frontend to show report
        }
    }

    fun startLyricsForegroundService(sessionId: String, total: Int) {
        val serviceIntent = Intent(this, LyricsForegroundService::class.java).apply {
            putExtra(LyricsForegroundService.EXTRA_SESSION_ID, sessionId)
            putExtra(LyricsForegroundService.EXTRA_PROGRESS, 0)
            putExtra(LyricsForegroundService.EXTRA_CURRENT_TRACK, "Starting...")
            putExtra(LyricsForegroundService.EXTRA_TOTAL, total)
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ContextCompat.startForegroundService(this, serviceIntent)
        } else {
            startService(serviceIntent)
        }
    }

    fun updateLyricsForegroundService(sessionId: String, progress: Int, currentTrack: String, total: Int) {
        val serviceIntent = Intent(this, LyricsForegroundService::class.java).apply {
            putExtra(LyricsForegroundService.EXTRA_SESSION_ID, sessionId)
            putExtra(LyricsForegroundService.EXTRA_PROGRESS, progress)
            putExtra(LyricsForegroundService.EXTRA_CURRENT_TRACK, currentTrack)
            putExtra(LyricsForegroundService.EXTRA_TOTAL, total)
        }
        startService(serviceIntent)
    }

    fun stopLyricsForegroundService(sessionId: String, found: Int, notFound: Int, errors: Int, total: Int) {
        val serviceIntent = Intent(this, LyricsForegroundService::class.java).apply {
            putExtra(LyricsForegroundService.EXTRA_SESSION_ID, sessionId)
            putExtra("found", found)
            putExtra("not_found", notFound)
            putExtra("errors", errors)
            putExtra("total", total)
            action = "complete"
        }
        startService(serviceIntent)
    }
}
