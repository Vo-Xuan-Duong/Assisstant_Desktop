package com.assisstant.voicesatellite

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingDeepLinkTest {
    private val token = "A".repeat(64)

    @Test
    fun validIpv4PayloadBuildsExpectedWebSocketAddress() {
        val result = PairingDeepLink.parseParts(
            scheme = "assd",
            endpoint = "p",
            desktopHost = "192.168.1.20",
            portRaw = "8765",
            tokenRaw = token,
        )

        assertTrue(result.isSuccess)
        assertEquals("ws://192.168.1.20:8765", result.getOrThrow().desktopAddress)
        assertEquals(token.lowercase(), result.getOrThrow().token)
    }

    @Test
    fun validDnsPayloadIsAcceptedCaseInsensitively() {
        val result = PairingDeepLink.parseParts(
            scheme = "ASSD",
            endpoint = "P",
            desktopHost = "assistant-pc.tailnet.example",
            portRaw = "443",
            tokenRaw = "0123456789abcdef".repeat(4),
        )

        assertTrue(result.isSuccess)
        assertEquals("ws://assistant-pc.tailnet.example:443", result.getOrThrow().desktopAddress)
    }

    @Test
    fun loopbackAndUnspecifiedIpv4AreRejected() {
        for (host in listOf("127.0.0.1", "127.12.34.56", "0.0.0.0", "localhost")) {
            val result = PairingDeepLink.parseParts("assd", "p", host, "8765", token)
            assertTrue("host should be rejected: $host", result.isFailure)
        }
    }

    @Test
    fun malformedHostsAreRejected() {
        for (host in listOf("256.1.1.1", "192.168.1", ".desktop", "desktop.", "desk..top", "-desktop.local", "desktop-.local", "desktop/path")) {
            val result = PairingDeepLink.parseParts("assd", "p", host, "8765", token)
            assertTrue("host should be rejected: $host", result.isFailure)
        }
    }

    @Test
    fun invalidPortIsRejected() {
        for (port in listOf("", "0", "65536", "not-a-port")) {
            val result = PairingDeepLink.parseParts("assd", "p", "192.168.1.20", port, token)
            assertTrue("port should be rejected: $port", result.isFailure)
        }
    }

    @Test
    fun malformedTokenIsRejected() {
        for (candidate in listOf("", "a".repeat(63), "a".repeat(65), "z".repeat(64))) {
            val result = PairingDeepLink.parseParts("assd", "p", "192.168.1.20", "8765", candidate)
            assertTrue("token should be rejected", result.isFailure)
        }
    }

    @Test
    fun wrongSchemeOrEndpointIsRejected() {
        assertTrue(
            PairingDeepLink.parseParts("https", "p", "192.168.1.20", "8765", token).isFailure,
        )
        assertTrue(
            PairingDeepLink.parseParts("assd", "other", "192.168.1.20", "8765", token).isFailure,
        )
    }
}
