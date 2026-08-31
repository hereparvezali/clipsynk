package com.parvez.clipsynk.core.codec

import com.parvez.clipsynk.core.models.Frame
import com.parvez.clipsynk.core.models.HandShake
import kotlinx.serialization.json.Json
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.InputStream
import java.io.OutputStream

object FrameCodec {
    val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        isLenient = true
    }

    fun encodeHandshake(handshake: HandShake): ByteArray {
        return json.encodeToString(HandShake.serializer(), handshake).toByteArray(Charsets.UTF_8)
    }

    fun decodeHandshake(bytes: ByteArray): HandShake {
        return json.decodeFromString(HandShake.serializer(), bytes.toString(Charsets.UTF_8))
    }

    fun encodeFrame(frame: Frame): ByteArray {
        return json.encodeToString(Frame.serializer(), frame).toByteArray(Charsets.UTF_8)
    }

    fun decodeFrame(bytes: ByteArray): Frame {
        return json.decodeFromString(Frame.serializer(), bytes.toString(Charsets.UTF_8))
    }

    fun writeHandshake(output: OutputStream, handshake: HandShake) {
        val payload = encodeHandshake(handshake)
        val dataOut = DataOutputStream(output)
        dataOut.writeInt(payload.size) // 4 bytes Big-Endian
        dataOut.write(payload)
        dataOut.flush()
    }

    fun readHandshake(input: InputStream): HandShake {
        val dataIn = DataInputStream(input)
        val length = dataIn.readInt()
        if (length < 0 || length > 1024 * 1024) {
            throw IllegalArgumentException("Invalid HandShake length: $length")
        }
        val buffer = ByteArray(length)
        dataIn.readFully(buffer)
        return decodeHandshake(buffer)
    }

    fun writeFrame(output: OutputStream, frame: Frame) {
        val payload = encodeFrame(frame)
        val dataOut = DataOutputStream(output)
        dataOut.writeInt(payload.size) // 4 bytes Big-Endian
        dataOut.write(payload)
        dataOut.flush()
    }

    fun readFrame(input: InputStream): Frame {
        val dataIn = DataInputStream(input)
        val length = dataIn.readInt()
        if (length < 0 || length > 50 * 1024 * 1024) {
            throw IllegalArgumentException("Invalid Frame length: $length")
        }
        val buffer = ByteArray(length)
        dataIn.readFully(buffer)
        return decodeFrame(buffer)
    }
}
