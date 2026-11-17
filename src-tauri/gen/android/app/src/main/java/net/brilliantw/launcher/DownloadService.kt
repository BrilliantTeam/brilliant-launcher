package net.brilliantw.launcher

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import androidx.core.app.NotificationCompat

class DownloadService : Service() {
  override fun onBind(intent: Intent?) = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (Build.VERSION.SDK_INT >= 26) {
      getSystemService(NotificationManager::class.java).createNotificationChannel(
        NotificationChannel(CHANNEL, intent?.getStringExtra(EXTRA_CHANNEL) ?: "Downloads", NotificationManager.IMPORTANCE_LOW)
      )
    }
    val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
    val notification = NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(android.R.drawable.stat_sys_download)
      .setContentTitle(intent?.getStringExtra(EXTRA_TITLE))
      .setOngoing(true)
      .setContentIntent(open)
      .build()
    if (Build.VERSION.SDK_INT >= 29) startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
    else startForeground(ID, notification)
    if (intent?.action == ACTION_STOP) {
      stopForeground(STOP_FOREGROUND_REMOVE)
      stopSelf()
    }
    return START_NOT_STICKY
  }

  companion object {
    const val CHANNEL = "downloads"
    const val EXTRA_TITLE = "title"
    const val EXTRA_CHANNEL = "channel"
    const val ACTION_STOP = "net.brilliantw.launcher.STOP_DOWNLOADS"
    private const val ID = 1
  }
}
