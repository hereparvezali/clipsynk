package com.parvez.clipsynk.core.models

import kotlinx.serialization.Serializable

@Serializable
data class Frame(
    val bytes: List<Int>,
    val timestamp: ULong,
    val hash: ULong
) {
    fun toByteArray(): ByteArray {
        val array = ByteArray(bytes.size)
        for (i in bytes.indices) {
            array[i] = bytes[i].toByte()
        }
        return array
    }

    fun toText(): String {
        return String(toByteArray(), Charsets.UTF_8)
    }

    companion object {
        fun fromByteArray(
            data: ByteArray,
            hash: ULong,
            timestamp: ULong = System.currentTimeMillis().toULong()
        ): Frame {
            val byteList = ArrayList<Int>(data.size)
            for (b in data) {
                byteList.add(b.toInt() and 0xFF)
            }
            return Frame(
                bytes = byteList,
                timestamp = timestamp,
                hash = hash
            )
        }

        fun fromText(
            text: String,
            hash: ULong,
            timestamp: ULong = System.currentTimeMillis().toULong()
        ): Frame {
            return fromByteArray(text.toByteArray(Charsets.UTF_8), hash, timestamp)
        }
    }
}
