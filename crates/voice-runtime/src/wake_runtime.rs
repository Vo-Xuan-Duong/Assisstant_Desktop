use std::time::Duration;

use serde::Serialize;
use tokio::{
    sync::{broadcast, mpsc, watch},
    time::{sleep, Instant},
};
use tracing::{debug, warn};

use crate::{
    wake::{WakeDetection, WakeWordDetector},
    MicrophoneConfig, MicrophoneStream,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WakeRuntimeState {
    Disabled,
    Starting,
    Listening,
    Suspended,
    Cooldown,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WakeRuntimeEvent {
    StateChanged {
        from: WakeRuntimeState,
        to: WakeRuntimeState,
    },
    Detected {
        detection: WakeDetection,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct WakeRuntimeConfig {
    pub microphone: MicrophoneConfig,
    /// Minimum detector quiet period after a successful wake before the runtime
    /// may reopen the microphone.
    pub detection_cooldown: Duration,
    /// Retry delay after the microphone/backend cannot be opened.
    pub error_retry_delay: Duration,
    pub enabled_on_start: bool,
}

impl Default for WakeRuntimeConfig {
    fn default() -> Self {
        Self {
            microphone: MicrophoneConfig::default(),
            detection_cooldown: Duration::from_millis(650),
            error_retry_delay: Duration::from_secs(3),
            enabled_on_start: false,
        }
    }
}

#[derive(Debug)]
enum WakeCommand {
    SetEnabled(bool),
    Suspend,
    Resume,
    Shutdown,
}

#[derive(Clone)]
pub struct WakeRuntimeHandle {
    commands: mpsc::Sender<WakeCommand>,
    state: watch::Receiver<WakeRuntimeState>,
    events: broadcast::Sender<WakeRuntimeEvent>,
}

impl WakeRuntimeHandle {
    pub async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        self.commands
            .send(WakeCommand::SetEnabled(enabled))
            .await
            .map_err(|_| "wake runtime is no longer running".to_owned())
    }

    pub async fn suspend(&self) -> Result<(), String> {
        self.commands
            .send(WakeCommand::Suspend)
            .await
            .map_err(|_| "wake runtime is no longer running".to_owned())
    }

    pub async fn resume(&self) -> Result<(), String> {
        self.commands
            .send(WakeCommand::Resume)
            .await
            .map_err(|_| "wake runtime is no longer running".to_owned())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.commands
            .send(WakeCommand::Shutdown)
            .await
            .map_err(|_| "wake runtime is no longer running".to_owned())
    }

    pub fn state(&self) -> WakeRuntimeState {
        *self.state.borrow()
    }

    pub fn subscribe_state(&self) -> watch::Receiver<WakeRuntimeState> {
        self.state.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WakeRuntimeEvent> {
        self.events.subscribe()
    }
}

pub fn spawn_wake_runtime(
    detector: Box<dyn WakeWordDetector>,
    config: WakeRuntimeConfig,
) -> WakeRuntimeHandle {
    let (command_tx, command_rx) = mpsc::channel(16);
    let initial_state = if config.enabled_on_start {
        WakeRuntimeState::Starting
    } else {
        WakeRuntimeState::Disabled
    };
    let (state_tx, state_rx) = watch::channel(initial_state);
    let (event_tx, _) = broadcast::channel(32);

    let handle = WakeRuntimeHandle {
        commands: command_tx,
        state: state_rx,
        events: event_tx.clone(),
    };

    tokio::spawn(run_worker(
        detector,
        config,
        command_rx,
        state_tx,
        event_tx,
    ));

    handle
}

async fn run_worker(
    mut detector: Box<dyn WakeWordDetector>,
    config: WakeRuntimeConfig,
    mut commands: mpsc::Receiver<WakeCommand>,
    state_tx: watch::Sender<WakeRuntimeState>,
    events: broadcast::Sender<WakeRuntimeEvent>,
) {
    let mut enabled = config.enabled_on_start;
    let mut suspended = false;
    let mut microphone: Option<MicrophoneStream> = None;
    let mut retry_at: Option<Instant> = None;
    let mut cooldown_until: Option<Instant> = None;

    loop {
        if !enabled {
            microphone = None;
            retry_at = None;
            cooldown_until = None;
            transition(&state_tx, &events, WakeRuntimeState::Disabled);

            match commands.recv().await {
                Some(WakeCommand::SetEnabled(true)) | Some(WakeCommand::Resume) => {
                    enabled = true;
                    suspended = false;
                    transition(&state_tx, &events, WakeRuntimeState::Starting);
                    continue;
                }
                Some(WakeCommand::Shutdown) | None => break,
                Some(WakeCommand::SetEnabled(false)) | Some(WakeCommand::Suspend) => continue,
            }
        }

        if suspended {
            microphone = None;
            retry_at = None;
            transition(&state_tx, &events, WakeRuntimeState::Suspended);

            match commands.recv().await {
                Some(WakeCommand::Resume) => {
                    suspended = false;
                    transition(&state_tx, &events, WakeRuntimeState::Starting);
                    continue;
                }
                Some(WakeCommand::SetEnabled(false)) => {
                    enabled = false;
                    continue;
                }
                Some(WakeCommand::Shutdown) | None => break,
                Some(WakeCommand::SetEnabled(true)) | Some(WakeCommand::Suspend) => continue,
            }
        }

        if let Some(until) = cooldown_until {
            if Instant::now() < until {
                transition(&state_tx, &events, WakeRuntimeState::Cooldown);
                tokio::select! {
                    command = commands.recv() => {
                        if apply_command(command, &mut enabled, &mut suspended) {
                            break;
                        }
                    }
                    _ = sleep(until.saturating_duration_since(Instant::now())) => {
                        cooldown_until = None;
                    }
                }
                continue;
            }
            cooldown_until = None;
        }

        if let Some(until) = retry_at {
            if Instant::now() < until {
                tokio::select! {
                    command = commands.recv() => {
                        if apply_command(command, &mut enabled, &mut suspended) {
                            break;
                        }
                    }
                    _ = sleep(until.saturating_duration_since(Instant::now())) => {
                        retry_at = None;
                    }
                }
                continue;
            }
            retry_at = None;
        }

        if microphone.is_none() {
            transition(&state_tx, &events, WakeRuntimeState::Starting);
            match MicrophoneStream::open_default(config.microphone) {
                Ok(stream) => {
                    debug!(device = %stream.info().device, "wake microphone opened");
                    microphone = Some(stream);
                    if let Err(error) = detector.reset() {
                        publish_error(&state_tx, &events, error.to_string());
                        microphone = None;
                        retry_at = Some(Instant::now() + config.error_retry_delay);
                        continue;
                    }
                    transition(&state_tx, &events, WakeRuntimeState::Listening);
                }
                Err(error) => {
                    publish_error(&state_tx, &events, error.to_string());
                    retry_at = Some(Instant::now() + config.error_retry_delay);
                    continue;
                }
            }
        }

        let stream = microphone.as_mut().expect("microphone initialized above");
        tokio::select! {
            command = commands.recv() => {
                if apply_command(command, &mut enabled, &mut suspended) {
                    break;
                }
                if !enabled || suspended {
                    microphone = None;
                }
            }
            chunk = stream.next_chunk() => {
                let Some(chunk) = chunk else {
                    publish_error(&state_tx, &events, "wake microphone stream ended unexpectedly".into());
                    microphone = None;
                    retry_at = Some(Instant::now() + config.error_retry_delay);
                    continue;
                };

                match detector.process(&chunk) {
                    Ok(Some(detection)) => {
                        // Release microphone ownership before publishing the event so
                        // the desktop can immediately begin a full voice turn.
                        microphone = None;
                        cooldown_until = Some(Instant::now() + config.detection_cooldown);
                        let _ = events.send(WakeRuntimeEvent::Detected { detection });
                        transition(&state_tx, &events, WakeRuntimeState::Cooldown);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        warn!(%error, "wake-word detector failed");
                        publish_error(&state_tx, &events, error.to_string());
                        microphone = None;
                        retry_at = Some(Instant::now() + config.error_retry_delay);
                    }
                }
            }
        }
    }

    transition(&state_tx, &events, WakeRuntimeState::Stopped);
}

/// Returns true when the worker should stop.
fn apply_command(
    command: Option<WakeCommand>,
    enabled: &mut bool,
    suspended: &mut bool,
) -> bool {
    match command {
        Some(WakeCommand::SetEnabled(value)) => {
            *enabled = value;
            if !value {
                *suspended = false;
            }
            false
        }
        Some(WakeCommand::Suspend) => {
            if *enabled {
                *suspended = true;
            }
            false
        }
        Some(WakeCommand::Resume) => {
            if *enabled {
                *suspended = false;
            }
            false
        }
        Some(WakeCommand::Shutdown) | None => true,
    }
}

fn publish_error(
    state: &watch::Sender<WakeRuntimeState>,
    events: &broadcast::Sender<WakeRuntimeEvent>,
    message: String,
) {
    transition(state, events, WakeRuntimeState::Error);
    let _ = events.send(WakeRuntimeEvent::Error { message });
}

fn transition(
    state: &watch::Sender<WakeRuntimeState>,
    events: &broadcast::Sender<WakeRuntimeEvent>,
    to: WakeRuntimeState,
) {
    let from = *state.borrow();
    if from == to {
        return;
    }
    state.send_replace(to);
    let _ = events.send(WakeRuntimeEvent::StateChanged { from, to });
}
