package com.assisstant.voicesatellite

import android.net.Uri

private const val PAIRING_SCHEME = "assd"
private const val PAIRING_HOST = "p"
private const val MAX_HOST_CHARS = 64

data class PairingImport(
    val desktopAddress: String,
    val token: String,
)

object PairingDeepLink {
    fun parse(uri: Uri?): Result<PairingImport> {
        if (uri == null) {
            return Result.failure(IllegalArgumentException("Pairing QR không có dữ liệu."))
        }

        return parseParts(
            scheme = uri.scheme,
            endpoint = uri.host,
            desktopHost = uri.getQueryParameter("h"),
            portRaw = uri.getQueryParameter("p"),
            tokenRaw = uri.getQueryParameter("t"),
        )
    }

    internal fun parseParts(
        scheme: String?,
        endpoint: String?,
        desktopHost: String?,
        portRaw: String?,
        tokenRaw: String?,
    ): Result<PairingImport> = runCatching {
        require(scheme.equals(PAIRING_SCHEME, ignoreCase = true)) {
            "Pairing QR không đúng định dạng Assistant."
        }
        require(endpoint.equals(PAIRING_HOST, ignoreCase = true)) {
            "Pairing QR không đúng endpoint Assistant."
        }

        val host = desktopHost?.trim().orEmpty()
        val portText = portRaw?.trim().orEmpty()
        val token = tokenRaw?.trim().orEmpty()

        require(isValidHost(host)) { "Địa chỉ desktop trong QR không hợp lệ." }
        val port = portText.toIntOrNull()
        require(port != null && port in 1..65535) { "Cổng desktop trong QR không hợp lệ." }
        require(token.length == 64 && token.all { it.isHexDigit() }) {
            "Pairing token trong QR không hợp lệ."
        }

        PairingImport(
            desktopAddress = "ws://$host:$port",
            token = token.lowercase(),
        )
    }

    private fun isValidHost(host: String): Boolean {
        if (host.isEmpty() || host.length > MAX_HOST_CHARS) return false
        if (!host.all { it.isAsciiHostChar() }) return false
        if (host.equals("localhost", true)) return false

        val looksNumeric = host.all { it in '0'..'9' || it == '.' }
        if (looksNumeric) {
            val parts = host.split('.')
            if (parts.size != 4) return false
            val octets = parts.map { part ->
                if (part.isEmpty() || part.length > 3) return false
                part.toIntOrNull() ?: return false
            }
            if (octets.any { it !in 0..255 }) return false
            if (octets.all { it == 0 } || octets[0] == 127) return false
            return true
        }

        if (host.startsWith('.') || host.endsWith('.') || host.contains("..")) return false
        return host.split('.').all { label ->
            label.isNotEmpty() &&
                !label.startsWith('-') &&
                !label.endsWith('-')
        }
    }

    private fun Char.isAsciiHostChar(): Boolean =
        this in 'a'..'z' || this in 'A'..'Z' || this in '0'..'9' || this == '-' || this == '.'

    private fun Char.isHexDigit(): Boolean =
        this in '0'..'9' || this in 'a'..'f' || this in 'A'..'F'
}
