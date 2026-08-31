package com.parvez.clipsynk.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import com.parvez.clipsynk.MainActivity
import com.parvez.clipsynk.core.models.Frame
import com.parvez.clipsynk.core.network.PeerInfo
import com.parvez.clipsynk.core.network.TransportManager
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow

class ClipSynkService : Service() {
    private val tag = "ClipSynkService"
    private var multicastLock: WifiManager.MulticastLock? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var serviceScope = CoroutineScope(Dispatchers.Default + SupervisorJob())
    private var transport: TransportManager? = null

    companion object {
        const val CHANNEL_ID = "clipsynk_channel"
        const val NOTIFICATION_ID = 101

        const val ACTION_START = "com.parvez.clipsynk.ACTION_START"
        const val ACTION_STOP = "com.parvez.clipsynk.ACTION_STOP"
        const val ACTION_TRIGGER_DISCOVERY = "com.parvez.clipsynk.ACTION_TRIGGER_DISCOVERY"

        private val _isServiceRunning = MutableStateFlow(false)
        val isServiceRunning: StateFlow<Boolean> = _isServiceRunning.asStateFlow()

        private val _connectedPeers = MutableStateFlow<List<PeerInfo>>(emptyList())
        val connectedPeers: StateFlow<List<PeerInfo>> = _connectedPeers.asStateFlow()

        private val _deviceId = MutableStateFlow<String?>(null)
        val deviceId: StateFlow<String?> = _deviceId.asStateFlow()

        private val _tcpPort = MutableStateFlow<Int?>(null)
        val tcpPort: StateFlow<Int?> = _tcpPort.asStateFlow()

        private val _latestRemoteText = MutableStateFlow<String?>(null)
        val latestRemoteText: StateFlow<String?> = _latestRemoteText.asStateFlow()

        var latestRemoteTimestamp: ULong = 0UL
        var latestRemoteHash: ULong = 0UL

        var activeTransport: TransportManager? = null
            private set
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()

        val notification = buildNotification("Starting LAN clipboard sync...", peerCount = 0)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            ServiceCompat.startForeground(
                this,
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }

        // Acquire WakeLock
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "ClipSynk::BackgroundNetworkLock"
        ).apply {
            acquire(24 * 60 * 60 * 1000L) // 24 hours max
        }

        // Acquire Wi-Fi Multicast Lock
        val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicastLock = wifiManager.createMulticastLock("clipsynk").apply {
            setReferenceCounted(true)
            acquire()
        }

        // Start Transport Engine
        val t = TransportManager(
            onRemoteFrame = { frame, peer ->
                handleIncomingFrame(frame, peer)
            }
        )
        transport = t
        activeTransport = t
        t.start(serviceScope)
        _deviceId.value = t.deviceId
        _tcpPort.value = t.tcpPort

        // Observe peers
        serviceScope.launch {
            t.peersState.collect { peers ->
                _connectedPeers.value = peers
                val message = if (peers.isEmpty()) {
                    "Listening for peers on LAN (Port ${t.tcpPort})..."
                } else {
                    "Connected to ${peers.size} peer(s) on LAN"
                }
                updateNotification(message, peers.size)
            }
        }

        _isServiceRunning.value = true
        Log.d(tag, "ClipSynkService started")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_TRIGGER_DISCOVERY -> {
                transport?.triggerDiscovery()
            }
        }
        return START_STICKY
    }

    private fun handleIncomingFrame(frame: Frame, peer: PeerInfo) {
        val text = frame.toText()
        latestRemoteTimestamp = frame.timestamp
        latestRemoteHash = frame.hash
        _latestRemoteText.value = text

        val preview = if (text.length > 8) text.take(8) + "..." else text
        updateNotification("Received: \"$preview\" - Tap to Sync", _connectedPeers.value.size)
    }

    private fun updateNotification(text: String, peerCount: Int) {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(NOTIFICATION_ID, buildNotification(text, peerCount))
    }

    private fun buildNotification(statusText: String, peerCount: Int): Notification {
        // Tap notification to open SyncActivity
        val syncIntent = Intent(this, SyncActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
        }
        val syncPendingIntent = PendingIntent.getActivity(
            this,
            1,
            syncIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Action: Open Main App
        val appIntent = Intent(this, MainActivity::class.java)
        val appPendingIntent = PendingIntent.getActivity(
            this,
            2,
            appIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        // Action: Stop Service
        val stopIntent = Intent(this, ClipSynkService::class.java).apply {
            action = ACTION_STOP
        }
        val stopPendingIntent = PendingIntent.getService(
            this,
            3,
            stopIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )

        val title = if (peerCount > 0) "ClipSynk ($peerCount Connected)" else "ClipSynk (Active)"

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(statusText)
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setOngoing(true)
            .setContentIntent(syncPendingIntent)
            .addAction(android.R.drawable.ic_menu_rotate, "Sync Now", syncPendingIntent)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Stop", stopPendingIntent)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "ClipSynk Foreground Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Keeps ClipSynk connected to LAN peers for real-time clipboard sync"
                setShowBadge(false)
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    override fun onDestroy() {
        Log.d(tag, "ClipSynkService stopping")
        _isServiceRunning.value = false
        _connectedPeers.value = emptyList()
        _deviceId.value = null
        _tcpPort.value = null

        transport?.stop()
        transport = null
        activeTransport = null

        serviceScope.cancel()

        if (multicastLock?.isHeld == true) {
            multicastLock?.release()
        }
        if (wakeLock?.isHeld == true) {
            wakeLock?.release()
        }

        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
}
