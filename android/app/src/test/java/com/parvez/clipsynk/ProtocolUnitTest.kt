package com.parvez.clipsynk

import com.parvez.clipsynk.core.codec.FrameCodec
import com.parvez.clipsynk.core.crypto.XXHash3
import com.parvez.clipsynk.core.models.Frame
import com.parvez.clipsynk.core.models.HandShake
import org.junit.Assert.*
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.util.UUID

class ProtocolUnitTest {

    @Test
    fun testHandshakeCodec() {
        val deviceId = UUID.randomUUID().toString()
        val original = HandShake(device_id = deviceId, tcp_port = 45678)

        val out = ByteArrayOutputStream()
        FrameCodec.writeHandshake(out, original)

        val bytes = out.toByteArray()
        assertTrue(bytes.size > 4) // 4-byte header + payload

        val inStream = ByteArrayInputStream(bytes)
        val decoded = FrameCodec.readHandshake(inStream)

        assertEquals(original.device_id, decoded.device_id)
        assertEquals(original.tcp_port, decoded.tcp_port)
    }

    @Test
    fun testFrameCodec() {
        val text = "Hello from Pure Kotlin ClipSynk!"
        val textBytes = text.toByteArray(Charsets.UTF_8)
        val hash = XXHash3.hash64(textBytes)
        val originalFrame = Frame.fromText(text, hash)

        val out = ByteArrayOutputStream()
        FrameCodec.writeFrame(out, originalFrame)

        val bytes = out.toByteArray()
        assertTrue(bytes.size > 4)

        val inStream = ByteArrayInputStream(bytes)
        val decoded = FrameCodec.readFrame(inStream)

        assertEquals(originalFrame.hash, decoded.hash)
        assertEquals(originalFrame.timestamp, decoded.timestamp)
        assertEquals(text, decoded.toText())
    }

    @Test
    fun testXXHash3RustCompatibility() {
        // Rust const_xxh3::xxh3_64 reference outputs:
        assertEquals(3244421341483603138UL, XXHash3.hash64("".toByteArray()))
        assertEquals(10760762337991515389UL, XXHash3.hash64("hello".toByteArray()))
        assertEquals(5297257993026170939UL, XXHash3.hash64("ClipSynk".toByteArray()))
        assertEquals(14879076941462221669UL, XXHash3.hash64("The quick brown fox jumps over the lazy dog".toByteArray()))
    }

    @Test
    fun testHash4jVsRustCompatibility() {
        val h = com.dynatrace.hash4j.hashing.Hashing.xxh3_64()
        assertEquals(3244421341483603138UL, h.hashBytesToLong("".toByteArray()).toULong())
        assertEquals(10760762337991515389UL, h.hashBytesToLong("hello".toByteArray()).toULong())
        assertEquals(5297257993026170939UL, h.hashBytesToLong("ClipSynk".toByteArray()).toULong())
        assertEquals(14879076941462221669UL, h.hashBytesToLong("The quick brown fox jumps over the lazy dog".toByteArray()).toULong())
    }
}
