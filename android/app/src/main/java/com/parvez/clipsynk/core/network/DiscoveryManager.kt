package com.parvez.clipsynk.core.network

import android.util.Log
import com.parvez.clipsynk.core.codec.FrameCodec
import com.parvez.clipsynk.core.models.HandShake
import kotlinx.coroutines.*
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface

class DiscoveryManager(
    private val deviceId: String,
    private val tcpPort: Int,
    private val broadcastPort: Int = 51515,
    private val onPeerDiscovered: (address: InetAddress, port: Int, peerDeviceId: String) -> Unit
) {
    private val tag = "DiscoveryManager"
    private var scope: CoroutineScope? = null
    private var isRunning = false

    fun start(parentScope: CoroutineScope) {
        if (isRunning) return
        isRunning = true
        scope = CoroutineScope(parentScope.coroutineContext + SupervisorJob())

        startBroadcaster()
        startListener()
    }

    fun stop() {
        isRunning = false
        scope?.cancel()
        scope = null
    }

    fun triggerBroadcast() {
        scope?.launch(Dispatchers.IO) {
            sendBroadcastPacket()
        }
    }

    private fun startBroadcaster() {
        scope?.launch(Dispatchers.IO) {
            Log.d(tag, "Starting UDP broadcaster on port $broadcastPort for device $deviceId, TCP: $tcpPort")
            // Send initial burst
            for (i in 0 until 3) {
                if (!isActive) break
                sendBroadcastPacket()
                delay(2000)
            }
            // Continuous broadcast every 30 seconds
            while (isActive) {
                sendBroadcastPacket()
                delay(30_000)
            }
        }
    }

    private fun sendBroadcastPacket() {
        try {
            val payload = FrameCodec.encodeHandshake(HandShake(deviceId, tcpPort))
            val socket = DatagramSocket()
            socket.broadcast = true
            val packet = DatagramPacket(
                payload,
                payload.size,
                InetAddress.getByName("255.255.255.255"),
                broadcastPort
            )
            socket.send(packet)
            socket.close()
            Log.d(tag, "Sent UDP broadcast: port=$tcpPort")
        } catch (e: Exception) {
            Log.w(tag, "Failed to send UDP broadcast: ${e.message}")
        }
    }

    private fun startListener() {
        scope?.launch(Dispatchers.IO) {
            var socket: DatagramSocket? = null
            try {
                socket = DatagramSocket(null).apply {
                    reuseAddress = true
                    broadcast = true
                    bind(InetSocketAddress(broadcastPort))
                }
                Log.d(tag, "Listening for UDP broadcasts on port $broadcastPort")

                val buffer = ByteArray(1024)
                while (isActive) {
                    val packet = DatagramPacket(buffer, buffer.size)
                    socket.receive(packet)

                    val length = packet.length
                    val data = buffer.copyOfRange(0, length)

                    try {
                        val handshake = FrameCodec.decodeHandshake(data)
                        if (handshake.device_id != deviceId && !isLocalIpAddress(packet.address)) {
                            Log.d(tag, "Discovered peer ${handshake.device_id} at ${packet.address}:${handshake.tcp_port}")
                            onPeerDiscovered(packet.address, handshake.tcp_port, handshake.device_id)
                        }
                    } catch (e: Exception) {
                        // Ignore malformed or unexpected UDP packets
                    }
                }
            } catch (e: Exception) {
                if (isActive) {
                    Log.e(tag, "Error in UDP listener: ${e.message}", e)
                }
            } finally {
                socket?.close()
            }
        }
    }

    private fun isLocalIpAddress(addr: InetAddress): Boolean {
        try {
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return false
            while (interfaces.hasMoreElements()) {
                val iface = interfaces.nextElement()
                val addresses = iface.inetAddresses
                while (addresses.hasMoreElements()) {
                    val address = addresses.nextElement()
                    if (address.hostAddress == addr.hostAddress) {
                        return true
                    }
                }
            }
        } catch (e: Exception) {
            // Ignore
        }
        return false
    }
}
