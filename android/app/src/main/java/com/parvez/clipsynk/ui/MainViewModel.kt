package com.parvez.clipsynk.ui

import android.app.Application
import android.content.Intent
import android.os.Build
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.parvez.clipsynk.core.network.PeerInfo
import com.parvez.clipsynk.service.ClipSynkService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

class MainViewModel(application: Application) : AndroidViewModel(application) {

    val isServiceRunning: StateFlow<Boolean> = ClipSynkService.isServiceRunning
    val connectedPeers: StateFlow<List<PeerInfo>> = ClipSynkService.connectedPeers
    val deviceId: StateFlow<String?> = ClipSynkService.deviceId
    val tcpPort: StateFlow<Int?> = ClipSynkService.tcpPort

    fun startService() {
        val context = getApplication<Application>()
        val intent = Intent(context, ClipSynkService::class.java).apply {
            action = ClipSynkService.ACTION_START
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.startForegroundService(intent)
        } else {
            context.startService(intent)
        }
    }

    fun stopService() {
        val context = getApplication<Application>()
        val intent = Intent(context, ClipSynkService::class.java).apply {
            action = ClipSynkService.ACTION_STOP
        }
        context.startService(intent)
    }

    fun triggerDiscovery() {
        viewModelScope.launch(Dispatchers.IO) {
            ClipSynkService.activeTransport?.triggerDiscovery()
        }
    }
}
