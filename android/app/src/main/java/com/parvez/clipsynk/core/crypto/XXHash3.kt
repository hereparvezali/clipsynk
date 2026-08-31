package com.parvez.clipsynk.core.crypto

import com.dynatrace.hash4j.hashing.Hashing

/**
 * 64-bit XXH3 (xxHash3) implementation using the standard Dynatrace hash4j library.
 * 100% bit-exact matching Rust's `xxhash_rust::const_xxh3::xxh3_64`.
 */
object XXHash3 {
    private val hasher = Hashing.xxh3_64()

    fun hash64(data: ByteArray): ULong {
        return hasher.hashBytesToLong(data).toULong()
    }
}
