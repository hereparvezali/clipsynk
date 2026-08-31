package com.parvez.clipsynk.service

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import com.parvez.clipsynk.core.crypto.XXHash3
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class SyncActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) return

        performSync()
        finish()
    }

    private fun performSync() {
        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val transport = ClipSynkService.activeTransport

        if (transport == null || !ClipSynkService.isServiceRunning.value) {
            Toast.makeText(this, "ClipSynk service is not running", Toast.LENGTH_SHORT).show()
            return
        }

        val clipDesc = clipboard.primaryClipDescription
        val androidTimestamp = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            clipDesc?.timestamp?.toULong() ?: 0UL
        } else {
            0UL
        }

        val pcTimestamp = ClipSynkService.latestRemoteTimestamp
        val remoteText = ClipSynkService.latestRemoteText.value

        // If remote text exists and is newer than local clipboard timestamp
        if (remoteText != null && pcTimestamp > androidTimestamp) {
            val clip = ClipData.newPlainText("ClipSynk", remoteText)
            clipboard.setPrimaryClip(clip)
            Toast.makeText(this, "Received from PC: ${remoteText.take(25)}...", Toast.LENGTH_SHORT).show()
        } else {
            // Android clipboard is newer or equal, send to PC
            val primaryClip = clipboard.primaryClip
            if (primaryClip != null && primaryClip.itemCount > 0) {
                val localText = primaryClip.getItemAt(0).text?.toString()
                if (!localText.isNullOrEmpty()) {
                    val localHash = XXHash3.hash64(localText.toByteArray(Charsets.UTF_8))
                    if (localHash != ClipSynkService.latestRemoteHash) {
                        CoroutineScope(Dispatchers.IO).launch {
                            transport.sendLocalText(localText)
                        }
                        Toast.makeText(this, "Sent to PC: ${localText.take(25)}...", Toast.LENGTH_SHORT).show()
                        return
                    }
                }
            }
            // If nothing new to send or receive
            if (remoteText != null) {
                val clip = ClipData.newPlainText("ClipSynk", remoteText)
                clipboard.setPrimaryClip(clip)
                Toast.makeText(this, "Synced clipboard", Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(this, "Clipboard is in sync", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
