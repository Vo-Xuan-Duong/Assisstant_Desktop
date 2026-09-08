package com.assisstant.voicesatellite

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class PairingSecretStore(context: Context) {
    private val appContext = context.applicationContext
    private val prefs = appContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    fun loadToken(): Result<String?> = runCatching {
        val ivEncoded = prefs.getString(KEY_IV, null) ?: return@runCatching null
        val ciphertextEncoded = prefs.getString(KEY_CIPHERTEXT, null) ?: return@runCatching null

        val iv = Base64.decode(ivEncoded, Base64.NO_WRAP)
        val ciphertext = Base64.decode(ciphertextEncoded, Base64.NO_WRAP)
        require(iv.isNotEmpty() && ciphertext.isNotEmpty()) { "Stored pairing credential is empty." }

        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        val plaintext = cipher.doFinal(ciphertext).toString(Charsets.UTF_8).trim()
        plaintext.takeIf { it.isNotEmpty() }
    }

    fun saveToken(rawToken: String): Result<Unit> = runCatching {
        val token = rawToken.trim()
        if (token.isEmpty()) {
            clear()
            return@runCatching
        }

        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val ciphertext = cipher.doFinal(token.toByteArray(Charsets.UTF_8))
        val iv = cipher.iv ?: error("Android Keystore did not provide an AES-GCM IV.")

        prefs.edit()
            .putString(KEY_IV, Base64.encodeToString(iv, Base64.NO_WRAP))
            .putString(KEY_CIPHERTEXT, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
            .apply()
    }

    fun clear() {
        prefs.edit()
            .remove(KEY_IV)
            .remove(KEY_CIPHERTEXT)
            .apply()
    }

    private fun getOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        val existing = keyStore.getKey(KEY_ALIAS, null) as? SecretKey
        if (existing != null) return existing

        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        val spec = KeyGenParameterSpec.Builder(
            KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setRandomizedEncryptionRequired(true)
            .build()
        generator.init(spec)
        return generator.generateKey()
    }

    companion object {
        private const val PREFS_NAME = "voice_satellite_secrets"
        private const val KEY_ALIAS = "assistant_voice_satellite_pairing_v1"
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val KEY_IV = "pairing_iv"
        private const val KEY_CIPHERTEXT = "pairing_ciphertext"
    }
}
