use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::oneshot;
use windows::{
    Win32::{
        Media::Speech::{
            ISpeechVoice, SVSFPurgeBeforeSpeak, SVSFlagsAsync, SpVoice,
            SpeechVoiceSpeakFlags,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
    core::{BSTR, Error as WindowsError},
};

const SAPI_POLL_MS: i32 = 40;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("text-to-speech input is empty")]
    EmptyText,
    #[error("text-to-speech backend is already speaking")]
    Busy,
    #[error("text-to-speech backend failed: {0}")]
    Backend(String),
    #[error("text-to-speech worker failed: {0}")]
    Worker(String),
}

#[derive(Debug, Clone, Copy)]
pub struct TtsConfig {
    /// Windows SAPI speaking rate. Values are clamped to -10..=10.
    pub rate: i32,
    /// Windows SAPI output volume. Values are clamped to 0..=100.
    pub volume: i32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            rate: 0,
            volume: 100,
        }
    }
}

#[async_trait]
pub trait TextToSpeech: Send + Sync {
    async fn speak(&self, text: &str) -> Result<(), TtsError>;
}

#[derive(Debug, Clone)]
pub struct WindowsSapiTts {
    config: TtsConfig,
    worker: Arc<Mutex<Option<mpsc::Sender<WorkerCommand>>>>,
}

impl Default for WindowsSapiTts {
    fn default() -> Self {
        Self::new(TtsConfig::default())
    }
}

impl WindowsSapiTts {
    pub fn new(config: TtsConfig) -> Self {
        Self {
            config,
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// Interrupt the active SAPI utterance without moving the COM voice across
    /// threads. The worker that owns ISpeechVoice receives the command and
    /// performs the purge itself.
    ///
    /// Returns false only when no worker exists or its command channel has
    /// already closed. Callers should still gate this by AssistantState::Speaking
    /// so an idle cancel never becomes sticky for a later utterance.
    pub fn cancel(&self) -> bool {
        let sender = self
            .worker
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().cloned());
        let Some(sender) = sender else {
            return false;
        };

        if sender.send(WorkerCommand::Cancel).is_ok() {
            return true;
        }

        self.clear_worker();
        false
    }

    fn worker_sender(&self) -> Result<mpsc::Sender<WorkerCommand>, TtsError> {
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| TtsError::Worker("SAPI worker state lock is poisoned".into()))?;
        if let Some(sender) = guard.as_ref() {
            return Ok(sender.clone());
        }

        let (sender, receiver) = mpsc::channel();
        let config = self.config;
        thread::Builder::new()
            .name("assistant-sapi-tts".into())
            .spawn(move || sapi_worker(config, receiver))
            .map_err(|error| TtsError::Worker(format!("cannot start SAPI worker: {error}")))?;
        *guard = Some(sender.clone());
        Ok(sender)
    }

    fn clear_worker(&self) {
        if let Ok(mut guard) = self.worker.lock() {
            *guard = None;
        }
    }

    fn send_speak(&self, text: String) -> Result<oneshot::Receiver<Result<(), TtsError>>, TtsError> {
        let (done, receiver) = oneshot::channel();
        let command = WorkerCommand::Speak { text, done };
        let sender = self.worker_sender()?;

        match sender.send(command) {
            Ok(()) => Ok(receiver),
            Err(error) => {
                // The owning thread may have exited after the sender was cached.
                // Recreate it once and preserve the original command/completion.
                self.clear_worker();
                let sender = self.worker_sender()?;
                sender
                    .send(error.0)
                    .map_err(|_| TtsError::Worker("SAPI worker stopped before accepting speech".into()))?;
                Ok(receiver)
            }
        }
    }
}

#[async_trait]
impl TextToSpeech for WindowsSapiTts {
    async fn speak(&self, text: &str) -> Result<(), TtsError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(TtsError::EmptyText);
        }

        self.send_speak(text.to_owned())?
            .await
            .map_err(|_| TtsError::Worker("SAPI worker stopped before completing speech".into()))?
    }
}

enum WorkerCommand {
    Speak {
        text: String,
        done: oneshot::Sender<Result<(), TtsError>>,
    },
    Cancel,
}

fn sapi_worker(config: TtsConfig, receiver: mpsc::Receiver<WorkerCommand>) {
    match create_voice(config) {
        Ok((voice, _com)) => worker_loop(&voice, receiver),
        Err(message) => reject_worker_commands(receiver, message),
    }
}

fn create_voice(config: TtsConfig) -> Result<(ISpeechVoice, ComGuard), String> {
    let initialize = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if initialize.is_err() {
        return Err(WindowsError::from_hresult(initialize).to_string());
    }
    let com = ComGuard;

    let voice: ISpeechVoice = unsafe { CoCreateInstance(&SpVoice, None, CLSCTX_ALL) }
        .map_err(|error| error.to_string())?;
    unsafe {
        voice
            .SetRate(config.rate.clamp(-10, 10))
            .map_err(|error| error.to_string())?;
        voice
            .SetVolume(config.volume.clamp(0, 100))
            .map_err(|error| error.to_string())?;
    }

    Ok((voice, com))
}

fn reject_worker_commands(receiver: mpsc::Receiver<WorkerCommand>, message: String) {
    while let Ok(command) = receiver.recv() {
        if let WorkerCommand::Speak { done, .. } = command {
            let _ = done.send(Err(TtsError::Backend(message.clone())));
        }
    }
}

fn worker_loop(voice: &ISpeechVoice, receiver: mpsc::Receiver<WorkerCommand>) {
    while let Ok(command) = receiver.recv() {
        match command {
            WorkerCommand::Speak { text, done } => {
                if !speak_on_worker(voice, &receiver, text, done) {
                    break;
                }
            }
            // An idle cancel is intentionally ignored. Quick only emits TTS
            // cancellation while the Assistant state is Speaking.
            WorkerCommand::Cancel => {}
        }
    }
}

fn speak_on_worker(
    voice: &ISpeechVoice,
    receiver: &mpsc::Receiver<WorkerCommand>,
    text: String,
    done: oneshot::Sender<Result<(), TtsError>>,
) -> bool {
    let text = BSTR::from(text);
    if let Err(error) = unsafe { voice.Speak(&text, SVSFlagsAsync) } {
        let _ = done.send(Err(TtsError::Backend(error.to_string())));
        return true;
    }

    loop {
        let completed = match unsafe { voice.WaitUntilDone(SAPI_POLL_MS) } {
            Ok(value) => value.as_bool(),
            Err(error) => {
                let _ = done.send(Err(TtsError::Backend(error.to_string())));
                return true;
            }
        };
        if completed {
            let _ = done.send(Ok(()));
            return true;
        }

        loop {
            match receiver.try_recv() {
                Ok(WorkerCommand::Cancel) => {
                    let result = purge_voice(voice).map_err(TtsError::Backend);
                    // User-initiated purge is a normal completion. Only a purge
                    // backend failure is surfaced as an error.
                    let _ = done.send(result);
                    return true;
                }
                Ok(WorkerCommand::Speak { done, .. }) => {
                    let _ = done.send(Err(TtsError::Busy));
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = purge_voice(voice);
                    let _ = done.send(Ok(()));
                    return false;
                }
            }
        }
    }
}

fn purge_voice(voice: &ISpeechVoice) -> Result<(), String> {
    let empty = BSTR::new();
    let flags = SpeechVoiceSpeakFlags(SVSFPurgeBeforeSpeak.0 | SVSFlagsAsync.0);
    unsafe { voice.Speak(&empty, flags) }
        .map(|_| ())
        .map_err(|error| error.to_string())
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
