package com.clipsynk

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import uniffi.clipsynk_ffi.ClipSynkEngine
import uniffi.clipsynk_ffi.MobileClipboardReceiver

class ClipSynkService : Service() {
    private var multicastLock: WifiManager.MulticastLock? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var engine: ClipSynkEngine? = null

    companion object {
        // Holds the latest frame received from the PC
        var latestRemoteText: String? = null
        var latestRemoteTimestamp: Long = 0L
        
        // Holds our Rust engine reference so the Activity can push local frames
        var activeEngine: ClipSynkEngine? = null
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(1, buildNotification("Syncing on LAN..."))

        // Acquire PARTIAL_WAKE_LOCK to keep CPU running when screen turns off
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "ClipSynk::BackgroundNetworkLock")
        wakeLock?.acquire()

        // Acquire MulticastLock to allow UDP discovery packets
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicastLock = wifiManager.createMulticastLock("clipsynk")
        multicastLock?.setReferenceCounted(true)
        multicastLock?.acquire()

        val receiver = object : MobileClipboardReceiver {
            override fun onRemoteFrame(hash: ULong, timestamp: ULong, bytes: ByteArray) {
                latestRemoteText = String(bytes)
                // We use the EXACT timestamp (Unix Epoch) sent from the PC
                latestRemoteTimestamp = timestamp.toLong()
                
                // Update notification so user knows new text arrived
                val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
                manager.notify(1, buildNotification("New text from PC. Tap Tile to Sync!"))
            }
        }

        // Start Rust engine
        engine = ClipSynkEngine.start(receiver)
        activeEngine = engine
    }

    override fun onDestroy() {
        multicastLock?.release()
        engine?.stop()
        engine = null
        activeEngine = null
        
        // Release the wake lock so the phone can sleep again when service stops
        if (wakeLock?.isHeld == true) {
            wakeLock?.release()
        }
        
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun buildNotification(text: String): Notification {
        val intent = Intent(this, SyncActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val pendingIntent = android.app.PendingIntent.getActivity(
            this, 0, intent, android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, "clipsynk_channel")
            .setContentTitle("ClipSynk")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setContentIntent(pendingIntent) // Tap notification to sync
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                "clipsynk_channel",
                "ClipSynk Service",
                NotificationManager.IMPORTANCE_LOW
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }
}
