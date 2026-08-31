package com.parvez.clipsynk.core.models

import kotlinx.serialization.Serializable

@Serializable
data class HandShake(
    val device_id: String,
    val tcp_port: Int
)
