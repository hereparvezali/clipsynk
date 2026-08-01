package com.clipsynk

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(64, 64, 64, 64)
        }

        val title = TextView(this).apply {
            text = "ClipSynk Android"
            textSize = 24f
            setPadding(0, 0, 0, 64)
        }

        val startBtn = Button(this).apply {
            text = "Start Background Service"
            setOnClickListener {
                val intent = Intent(this@MainActivity, ClipSynkService::class.java)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    startForegroundService(intent)
                } else {
                    startService(intent)
                }
            }
        }

        val stopBtn = Button(this).apply {
            text = "Stop Background Service"
            setOnClickListener {
                stopService(Intent(this@MainActivity, ClipSynkService::class.java))
            }
        }

        layout.addView(title)
        layout.addView(startBtn)
        layout.addView(stopBtn)

        setContentView(layout)
    }
}
