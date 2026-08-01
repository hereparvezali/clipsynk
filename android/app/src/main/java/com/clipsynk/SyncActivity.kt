package com.clipsynk

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.widget.Toast

class SyncActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) return

        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val engine = ClipSynkService.activeEngine

        if (engine == null) {
            Toast.makeText(this, "ClipSynk is not running!", Toast.LENGTH_SHORT).show()
            finish()
            return
        }

        // We MUST read the clipboard here because Android 10+ blocks access in onCreate()
        val clipDesc = clipboard.primaryClipDescription
        
        // Get the Android clipboard item's timestamp (Unix Epoch millis)
        // Note: This requires API 33+. If unavailable, defaults to 0.
        val androidTimestamp = clipDesc?.timestamp ?: 0L
        val pcTimestamp = ClipSynkService.latestRemoteTimestamp

        if (androidTimestamp > pcTimestamp) {
            // Android clipboard's item is newer! Send it to the PC.
            val item = clipboard.primaryClip?.getItemAt(0)
            val text = item?.text?.toString()
            if (text != null) {
                engine.sendLocalFrame(text.toByteArray())
                Toast.makeText(this, "Sent to PC", Toast.LENGTH_SHORT).show()
            }
        } else {
            // PC's item is newer! Receive it to Android.
            val remoteText = ClipSynkService.latestRemoteText
            if (remoteText != null) {
                val clip = ClipData.newPlainText("ClipSynk", remoteText)
                clipboard.setPrimaryClip(clip)
                Toast.makeText(this, "Received from PC", Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(this, "Nothing to sync", Toast.LENGTH_SHORT).show()
            }
        }

        // Instantly close the invisible activity
        finish()
    }
}
