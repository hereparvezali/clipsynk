package com.parvez.clipsynk.core.network

import android.util.Log
import com.parvez.clipsynk.core.codec.FrameCodec
import com.parvez.clipsynk.core.crypto.XXHash3
import com.parvez.clipsynk.core.models.Frame
import com.parvez.clipsynk.core.models.HandShake
import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.util.UUID

data class PeerInfo(
    val deviceId: String,
    val address: String,
    val port: Int,
    val connectedAt: Long = System.currentTimeMillis()
)

class TransportManager(
    val deviceId: String = UUID.randomUUID().toString(),
    private val onRemoteFrame: (Frame, PeerInfo) -> Unit
) {
    private val tag = "TransportManager"
    private var serverSocket: ServerSocket? = null
    var tcpPort: Int = 0
        private set

    private var scope: CoroutineScope? = null
    private var discoveryManager: DiscoveryManager? = null
    private val peersMutex = Mutex()
    private val activePeersMap = mutableMapOf<String, PeerConnection>()

    private val _peersState = MutableStateFlow<List<PeerInfo>>(emptyList())
    val peersState: StateFlow<List<PeerInfo>> = _peersState.asStateFlow()

    private class PeerConnection(
        val peerInfo: PeerInfo,
        val socket: Socket,
        val outChannel: Channel<Frame>
    )

    fun start(parentScope: CoroutineScope) {
        val newScope = CoroutineScope(parentScope.coroutineContext + SupervisorJob())
        scope = newScope

        try {
            val server = ServerSocket(0)
            serverSocket = server
            tcpPort = server.localPort
            Log.d(tag, "TCP Server bound to port: $tcpPort for device: $deviceId")

            // Accept incoming connections
            newScope.launch(Dispatchers.IO) {
                while (isActive && !server.isClosed) {
                    try {
                        val clientSocket = server.accept()
                        handleConnection(clientSocket, isOutbound = false)
                    } catch (e: Exception) {
                        if (!server.isClosed && isActive) {
                            Log.w(tag, "Server accept error: ${e.message}")
                        }
                    }
                }
            }

            // Start UDP discovery
            discoveryManager = DiscoveryManager(
                deviceId = deviceId,
                tcpPort = tcpPort,
                onPeerDiscovered = { address, port, peerDeviceId ->
                    newScope.launch(Dispatchers.IO) {
                        connectToPeer(address, port, peerDeviceId)
                    }
                }
            ).apply {
                start(newScope)
            }
        } catch (e: Exception) {
            Log.e(tag, "Failed to start TCP Server: ${e.message}", e)
        }
    }

    fun stop() {
        discoveryManager?.stop()
        discoveryManager = null

        scope?.launch(Dispatchers.IO) {
            peersMutex.withLock {
                activePeersMap.values.forEach { peer ->
                    try {
                        peer.socket.close()
                    } catch (e: Exception) {
                        // Ignore
                    }
                }
                activePeersMap.clear()
                updatePeersState()
            }
        }

        try {
            serverSocket?.close()
        } catch (e: Exception) {
            // Ignore
        }
        serverSocket = null
        scope?.cancel()
        scope = null
    }

    fun triggerDiscovery() {
        discoveryManager?.triggerBroadcast()
    }

    suspend fun sendLocalText(text: String): Frame {
        val bytes = text.toByteArray(Charsets.UTF_8)
        val hash = XXHash3.hash64(bytes)
        val frame = Frame.fromByteArray(bytes, hash)
        sendLocalFrame(frame)
        return frame
    }

    suspend fun sendLocalFrame(frame: Frame) {
        peersMutex.withLock {
            val toRemove = mutableListOf<String>()
            for ((id, peer) in activePeersMap) {
                val result = peer.outChannel.trySend(frame)
                if (result.isClosed) {
                    toRemove.add(id)
                }
            }
            if (toRemove.isNotEmpty()) {
                toRemove.forEach { activePeersMap.remove(it) }
                updatePeersState()
            }
        }
    }

    private suspend fun connectToPeer(address: InetAddress, port: Int, peerDeviceId: String) {
        peersMutex.withLock {
            if (activePeersMap.containsKey(peerDeviceId)) {
                return // Already connected
            }
        }

        try {
            val socket = Socket()
            socket.connect(InetSocketAddress(address, port), 4000)
            handleConnection(socket, isOutbound = true)
        } catch (e: Exception) {
            Log.w(tag, "Failed to connect to discovered peer $peerDeviceId at $address:$port - ${e.message}")
        }
    }

    private fun handleConnection(socket: Socket, isOutbound: Boolean) {
        scope?.launch(Dispatchers.IO) {
            var peerId: String? = null
            try {
                socket.tcpNoDelay = true
                val input = BufferedInputStream(socket.getInputStream())
                val output = BufferedOutputStream(socket.getOutputStream())

                // 1. Handshake exchange
                val localHandshake = HandShake(deviceId, tcpPort)
                FrameCodec.writeHandshake(output, localHandshake)

                val remoteHandshake = FrameCodec.readHandshake(input)
                peerId = remoteHandshake.device_id

                if (peerId == deviceId) {
                    socket.close()
                    return@launch
                }

                val peerInfo = PeerInfo(
                    deviceId = remoteHandshake.device_id,
                    address = socket.inetAddress.hostAddress ?: "unknown",
                    port = remoteHandshake.tcp_port
                )

                val outChannel = Channel<Frame>(Channel.BUFFERED)
                val peerConn = PeerConnection(peerInfo, socket, outChannel)

                var accepted = false
                peersMutex.withLock {
                    if (!activePeersMap.containsKey(remoteHandshake.device_id)) {
                        activePeersMap[remoteHandshake.device_id] = peerConn
                        updatePeersState()
                        accepted = true
                        Log.d(tag, "Peer connected: ${remoteHandshake.device_id} ($peerInfo)")
                    }
                }

                if (!accepted) {
                    socket.close()
                    return@launch
                }

                // 2. Writer Job
                val writerJob = launch {
                    try {
                        for (frame in outChannel) {
                            FrameCodec.writeFrame(output, frame)
                            Log.d(tag, "Sent frame to peer: ${remoteHandshake.device_id}")
                        }
                    } catch (e: Exception) {
                        Log.d(tag, "Writer closed for ${remoteHandshake.device_id}: ${e.message}")
                    }
                }

                // 3. Reader Loop
                try {
                    while (isActive && !socket.isClosed) {
                        val frame = FrameCodec.readFrame(input)
                        Log.d(tag, "Received frame from peer: ${remoteHandshake.device_id}")
                        onRemoteFrame(frame, peerInfo)
                    }
                } catch (e: Exception) {
                    Log.d(tag, "Reader ended for ${remoteHandshake.device_id}: ${e.message}")
                } finally {
                    writerJob.cancel()
                }

            } catch (e: Exception) {
                Log.w(tag, "Connection error: ${e.message}")
            } finally {
                peerId?.let { id ->
                    peersMutex.withLock {
                        activePeersMap.remove(id)?.let {
                            try {
                                it.socket.close()
                            } catch (e: Exception) {
                                // Ignore
                            }
                        }
                        updatePeersState()
                        Log.d(tag, "Peer disconnected: $id")
                    }
                }
            }
        }
    }

    private fun updatePeersState() {
        _peersState.value = activePeersMap.values.map { it.peerInfo }
    }
}
