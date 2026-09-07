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
    fun parse(uri: Uri?): Result<PairingImport> = runCatching {
        require(uri != null) { "Pairing QR không có dữ liệu." }
        require(uri.scheme.equals(PAIRING_SCHEME, ignoreCase = true)) {
            "Pairing QR không đúng định dạng Assistant."
        }
        require(uri.host.equals(PAIRING_HOST, ignoreCase = true)) {
            "Pairing QR không đúng endpoint Assistant."
        }

        val host = uri.getQueryParameter("h")?.trim().orEmpty()
        val portRaw = uri.getQueryParameter("p")?.trim().orEmpty()
        val token = uri.getQueryParameter("t")?.trim().orEmpty()

        require(isValidHost(host)) { "Địa chỉ desktop trong QR không hợp lệ." }
        val port = portRaw.toIntOrNull()
        require(port != null && port in 1..65535) { "Cổng desktop trong QR không hợp lệ." }
        require(token.length == 64 && token.all(Char::isHexDigit)) {
            "Pairing token trong QR không hợp lệ."
        }

        PairingImport(
            desktopAddress = "ws://$host:$port",
            token = token.lowercase(),
        )
    }

    private fun isValidHost(host: String): Boolean {
        if (host.isEmpty() || host.length > MAX_HOST_CHARS) return false
        if (host == "0.0.0.0" || host == "127.0.0.1" || host.equals("localhost", true)) {
            return false
        }
        if (host.startsWith('.') || host.endsWith('.') || host.contains("..")) return false
        return host.all { it.isLetterOrDigit() || it == '-' || it == '.' }
    }

    private fun Char.isHexDigit(): Boolean =
        this in '0'..'9' || this in 'a'..'f' || this in 'A'..'F'
}
