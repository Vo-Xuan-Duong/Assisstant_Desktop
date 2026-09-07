package com.assisstant.voicesatellite

import android.Manifest
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.weight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    private lateinit var satelliteClient: SatelliteClient
    private lateinit var speechController: SpeechController

    private var desktopAddress by mutableStateOf("ws://192.168.1.20:8765")
    private var pairingToken by mutableStateOf("")
    private var recognitionLanguage by mutableStateOf("vi-VN")
    private var responseLanguage by mutableStateOf("vi")
    private var preferOnDevice by mutableStateOf(true)

    private var connected by mutableStateOf(false)
    private var connectionStatus by mutableStateOf("Chưa kết nối")
    private var listening by mutableStateOf(false)
    private var partialText by mutableStateOf("")
    private var lastResponse by mutableStateOf("")
    private var statusMessage by mutableStateOf("")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        loadSettings()

        satelliteClient = SatelliteClient(
            onConnectionChanged = { ready, status ->
                connected = ready
                connectionStatus = status
            },
            onTurnState = { state ->
                statusMessage = when (state) {
                    "processing" -> "Desktop đang xử lý lệnh…"
                    "speaking" -> "Desktop đang trả lời…"
                    else -> state
                }
            },
            onResponse = { text, ttsError ->
                lastResponse = text
                statusMessage = if (ttsError == null) {
                    "Hoàn tất"
                } else {
                    "Đã xử lý, nhưng desktop TTS gặp lỗi: $ttsError"
                }
            },
            onError = { message -> statusMessage = message },
        )

        speechController = SpeechController(
            context = this,
            onListeningChanged = { listening = it },
            onPartialText = { partialText = it },
            onFinalText = { finalText ->
                partialText = finalText
                if (!satelliteClient.sendCommand(finalText, responseLanguage)) {
                    statusMessage = "Chưa kết nối với desktop hoặc kết nối chưa sẵn sàng."
                } else {
                    statusMessage = "Đã gửi lệnh cho desktop."
                }
            },
            onError = { message -> statusMessage = message },
        )

        setContent {
            MaterialTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    SatelliteScreen()
                }
            }
        }
    }

    override fun onDestroy() {
        speechController.destroy()
        satelliteClient.shutdown()
        super.onDestroy()
    }

    @Composable
    private fun SatelliteScreen() {
        val microphonePermission = rememberLauncherForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { granted ->
            if (granted) {
                startSpeechRecognition()
            } else {
                statusMessage = "Cần quyền microphone để nhận lệnh giọng nói."
            }
        }

        DisposableEffect(Unit) {
            onDispose {
                saveSettings()
            }
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text("Assistant Voice Satellite", style = MaterialTheme.typography.headlineSmall)
            Text(
                "Điện thoại nhận giọng nói; desktop xử lý lệnh và phát câu trả lời.",
                style = MaterialTheme.typography.bodyMedium,
            )

            OutlinedTextField(
                value = desktopAddress,
                onValueChange = { desktopAddress = it },
                label = { Text("Desktop WebSocket") },
                placeholder = { Text("ws://192.168.1.20:8765") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            OutlinedTextField(
                value = pairingToken,
                onValueChange = { pairingToken = it },
                label = { Text("Pairing token") },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                modifier = Modifier.fillMaxWidth(),
            )

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Button(
                    onClick = {
                        saveSettings()
                        satelliteClient.connect(desktopAddress, pairingToken)
                    },
                ) {
                    Text(if (connected) "Kết nối lại" else "Kết nối")
                }
                OutlinedButton(
                    onClick = {
                        satelliteClient.close()
                        connected = false
                        connectionStatus = "Đã ngắt kết nối"
                    },
                    enabled = connected,
                ) {
                    Text("Ngắt")
                }
            }
            Text("Trạng thái: $connectionStatus")

            HorizontalDivider()

            Text("Ngôn ngữ nhận dạng", style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = recognitionLanguage == "vi-VN",
                    onClick = { recognitionLanguage = "vi-VN" },
                    label = { Text("Tiếng Việt") },
                )
                FilterChip(
                    selected = recognitionLanguage == "en-US",
                    onClick = { recognitionLanguage = "en-US" },
                    label = { Text("English") },
                )
            }

            Text("Ngôn ngữ desktop phản hồi", style = MaterialTheme.typography.titleMedium)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = responseLanguage == "vi",
                    onClick = { responseLanguage = "vi" },
                    label = { Text("VI") },
                )
                FilterChip(
                    selected = responseLanguage == "en",
                    onClick = { responseLanguage = "en" },
                    label = { Text("EN") },
                )
                FilterChip(
                    selected = responseLanguage == "auto",
                    onClick = { responseLanguage = "auto" },
                    label = { Text("Auto") },
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text("Ưu tiên nhận dạng on-device")
                    Text(
                        "Nếu thiết bị hỗ trợ; nếu không sẽ dùng dịch vụ SpeechRecognizer hệ thống.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Switch(
                    checked = preferOnDevice,
                    onCheckedChange = { preferOnDevice = it },
                )
            }

            Spacer(Modifier.height(4.dp))
            Button(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(58.dp),
                enabled = connected,
                onClick = {
                    if (listening) {
                        speechController.stop()
                    } else if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) {
                        startSpeechRecognition()
                    } else {
                        microphonePermission.launch(Manifest.permission.RECORD_AUDIO)
                    }
                },
            ) {
                Text(if (listening) "Dừng nghe" else "Nói với Assistant")
            }

            if (partialText.isNotBlank()) {
                Text("Bạn nói", style = MaterialTheme.typography.titleSmall)
                Text(partialText, style = MaterialTheme.typography.bodyLarge)
            }

            if (statusMessage.isNotBlank()) {
                Text(statusMessage, style = MaterialTheme.typography.bodyMedium)
            }

            if (lastResponse.isNotBlank()) {
                HorizontalDivider()
                Text("Desktop trả lời", style = MaterialTheme.typography.titleSmall)
                Text(lastResponse, style = MaterialTheme.typography.bodyLarge)
            }
        }
    }

    private fun startSpeechRecognition() {
        if (!satelliteClient.isReady()) {
            statusMessage = "Hãy kết nối với desktop trước."
            return
        }
        saveSettings()
        lastResponse = ""
        statusMessage = if (preferOnDevice) {
            "Đang mở bộ nhận dạng giọng nói…"
        } else {
            "Đang nghe…"
        }
        speechController.start(recognitionLanguage, preferOnDevice)
    }

    private fun loadSettings() {
        val prefs = getSharedPreferences("voice_satellite", MODE_PRIVATE)
        desktopAddress = prefs.getString("desktop_address", desktopAddress) ?: desktopAddress
        pairingToken = prefs.getString("pairing_token", "") ?: ""
        recognitionLanguage = prefs.getString("recognition_language", "vi-VN") ?: "vi-VN"
        responseLanguage = prefs.getString("response_language", "vi") ?: "vi"
        preferOnDevice = prefs.getBoolean("prefer_on_device", true)
    }

    private fun saveSettings() {
        getSharedPreferences("voice_satellite", MODE_PRIVATE)
            .edit()
            .putString("desktop_address", desktopAddress.trim())
            .putString("pairing_token", pairingToken.trim())
            .putString("recognition_language", recognitionLanguage)
            .putString("response_language", responseLanguage)
            .putBoolean("prefer_on_device", preferOnDevice)
            .apply()
    }
}
