package com.milesseventh.deepkick.bridge

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

object Notifications {
    const val CHANNEL_PROGRESS = "deepkick_progress"
    const val CHANNEL_DONE = "deepkick_done"
    const val ID_PROGRESS = 41
    const val ID_DONE = 42

    /** The app's main activity, remembered so notifications can reopen it. */
    @Volatile var activityClass: Class<*>? = null

    fun ensureChannels(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_PROGRESS, "Processing progress", NotificationManager.IMPORTANCE_LOW).apply {
                description = "Shown while lyrics are being fetched or removed"
                setShowBadge(false)
            }
        )
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_DONE, "Reports", NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = "Posted when a queue is finished"
            }
        )
    }

    private fun canNotify(context: Context): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
        return ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
    }

    private fun openApp(context: Context, action: String): PendingIntent? {
        val cls = activityClass
        val intent = if (cls != null) {
            Intent(context, cls)
        } else {
            context.packageManager.getLaunchIntentForPackage(context.packageName) ?: return null
        }
        intent.action = Intent.ACTION_MAIN
        intent.addCategory(Intent.CATEGORY_LAUNCHER)
        intent.addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_NEW_TASK)
        intent.putExtra(DeepkickPlugin.EXTRA_ACTION, action)
        var flags = PendingIntent.FLAG_UPDATE_CURRENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) flags = flags or PendingIntent.FLAG_IMMUTABLE
        return PendingIntent.getActivity(context, action.hashCode(), intent, flags)
    }

    fun progress(context: Context, title: String, text: String, max: Int, progress: Int): Notification {
        return NotificationCompat.Builder(context, CHANNEL_PROGRESS)
            .setSmallIcon(R.drawable.ic_deepkick_notification)
            .setContentTitle(title)
            .setContentText(text)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setSilent(true)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .setProgress(max, progress, max <= 0)
            .setContentIntent(openApp(context, "queue"))
            .build()
    }

    fun notifyProgress(context: Context, title: String, text: String, max: Int, progress: Int) {
        if (!canNotify(context)) return
        try {
            NotificationManagerCompat.from(context).notify(ID_PROGRESS, progress(context, title, text, max, progress))
        } catch (_: SecurityException) {
        }
    }

    fun notifyDone(context: Context, title: String, text: String) {
        if (!canNotify(context)) return
        val notification = NotificationCompat.Builder(context, CHANNEL_DONE)
            .setSmallIcon(R.drawable.ic_deepkick_notification)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_STATUS)
            .setContentIntent(openApp(context, "report"))
            .build()
        try {
            NotificationManagerCompat.from(context).notify(ID_DONE, notification)
        } catch (_: SecurityException) {
        }
    }
}
