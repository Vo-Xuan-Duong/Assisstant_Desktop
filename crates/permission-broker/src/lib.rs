use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use assistant_common::ToolRisk;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Mutex},
    time::timeout,
};
use uuid::Uuid;

pub const ENV_BROKER_ADDR: &str = "ASSISTANT_PERMISSION_BROKER_ADDR";
pub const ENV_BROKER_SECRET: &str = "ASSISTANT_PERMISSION_BROKER_SECRET";

const MAX_MESSAGE_BYTES: usize = 64 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const RESPONSE_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct BrokerEndpoint {
    pub address: SocketAddr,
    secret: String,
}

impl BrokerEndpoint {
    pub fn environment(&self) -> [(String, String); 2] {
        [
            (ENV_BROKER_ADDR.to_owned(), self.address.to_string()),
            (ENV_BROKER_SECRET.to_owned(), self.secret.clone()),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub request_id: Uuid,
    pub tool_name: String,
    pub risk: ToolRisk,
    pub arguments: Value,
}

impl PermissionRequest {
    pub fn new(tool_name: impl Into<String>, risk: ToolRisk, arguments: Value) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            tool_name: tool_name.into(),
            risk,
            arguments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDecision {
    AllowOnce,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestEnvelope {
    secret: String,
    request: PermissionRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResponseEnvelope {
    request_id: Uuid,
    decision: UserDecision,
    error: Option<String>,
}

#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("permission broker environment is incomplete")]
    MissingEnvironment,
    #[error("invalid permission broker address: {0}")]
    InvalidAddress(String),
    #[error("permission broker I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("permission broker JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("permission broker request timed out")]
    Timeout,
    #[error("permission broker channel is closed")]
    ChannelClosed,
    #[error("permission broker rejected request: {0}")]
    Rejected(String),
    #[error("permission broker response id did not match request")]
    ResponseMismatch,
}

#[derive(Clone)]
pub struct BrokerClient {
    endpoint: BrokerEndpoint,
    response_timeout: Duration,
}

impl BrokerClient {
    pub fn from_environment(response_timeout: Duration) -> Result<Self, BrokerError> {
        let address = std::env::var(ENV_BROKER_ADDR).map_err(|_| BrokerError::MissingEnvironment)?;
        let secret = std::env::var(ENV_BROKER_SECRET).map_err(|_| BrokerError::MissingEnvironment)?;
        if secret.trim().is_empty() {
            return Err(BrokerError::MissingEnvironment);
        }
        let address = address
            .parse::<SocketAddr>()
            .map_err(|_| BrokerError::InvalidAddress(address))?;
        if !address.ip().is_loopback() {
            return Err(BrokerError::InvalidAddress(
                "permission broker must use a loopback address".into(),
            ));
        }

        Ok(Self {
            endpoint: BrokerEndpoint { address, secret },
            response_timeout,
        })
    }

    pub async fn request(
        &self,
        request: PermissionRequest,
    ) -> Result<UserDecision, BrokerError> {
        let request_id = request.request_id;
        let stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(self.endpoint.address))
            .await
            .map_err(|_| BrokerError::Timeout)??;
        let (read_half, mut write_half) = stream.into_split();

        let envelope = RequestEnvelope {
            secret: self.endpoint.secret.clone(),
            request,
        };
        let mut payload = serde_json::to_vec(&envelope)?;
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(BrokerError::Rejected(
                "permission request exceeds broker message limit".into(),
            ));
        }
        payload.push(b'\n');
        write_half.write_all(&payload).await?;
        write_half.flush().await?;

        let mut reader = BufReader::new(read_half);
        let mut line = String::new();
        let read = timeout(self.response_timeout + RESPONSE_GRACE, reader.read_line(&mut line))
            .await
            .map_err(|_| BrokerError::Timeout)??;
        if read == 0 {
            return Err(BrokerError::ChannelClosed);
        }
        if line.len() > MAX_MESSAGE_BYTES {
            return Err(BrokerError::Rejected(
                "permission response exceeds broker message limit".into(),
            ));
        }

        let response: ResponseEnvelope = serde_json::from_str(&line)?;
        if response.request_id != request_id {
            return Err(BrokerError::ResponseMismatch);
        }
        if let Some(error) = response.error {
            return Err(BrokerError::Rejected(error));
        }
        Ok(response.decision)
    }
}

#[derive(Clone)]
pub struct BrokerHandle {
    endpoint: BrokerEndpoint,
    pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<UserDecision>>>>,
}

impl BrokerHandle {
    pub fn endpoint(&self) -> &BrokerEndpoint {
        &self.endpoint
    }

    pub async fn respond(
        &self,
        request_id: Uuid,
        decision: UserDecision,
    ) -> Result<(), BrokerError> {
        let sender = self.pending.lock().await.remove(&request_id);
        let Some(sender) = sender else {
            return Err(BrokerError::Rejected(
                "permission request is no longer pending".into(),
            ));
        };
        sender.send(decision).map_err(|_| BrokerError::ChannelClosed)
    }
}

pub async fn bind_local(
    response_timeout: Duration,
) -> Result<(BrokerHandle, mpsc::Receiver<PermissionRequest>), BrokerError> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let endpoint = BrokerEndpoint {
        address,
        // Two independent UUIDv4 values provide a short-lived 256-bit-ish
        // opaque session credential without persisting anything to disk.
        secret: format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
    };
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let (request_tx, request_rx) = mpsc::channel(8);

    let handle = BrokerHandle {
        endpoint: endpoint.clone(),
        pending: Arc::clone(&pending),
    };

    tokio::spawn(async move {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                break;
            };
            if !peer.ip().is_loopback() {
                continue;
            }

            let endpoint = endpoint.clone();
            let pending = Arc::clone(&pending);
            let request_tx = request_tx.clone();
            tokio::spawn(async move {
                let _ = handle_connection(
                    stream,
                    endpoint,
                    pending,
                    request_tx,
                    response_timeout,
                )
                .await;
            });
        }
    });

    Ok((handle, request_rx))
}

async fn handle_connection(
    stream: TcpStream,
    endpoint: BrokerEndpoint,
    pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<UserDecision>>>>,
    request_tx: mpsc::Sender<PermissionRequest>,
    response_timeout: Duration,
) -> Result<(), BrokerError> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let read = reader.read_line(&mut line).await?;
    if read == 0 || line.len() > MAX_MESSAGE_BYTES {
        return Ok(());
    }

    let envelope: RequestEnvelope = match serde_json::from_str(&line) {
        Ok(envelope) => envelope,
        Err(_) => return Ok(()),
    };
    let request_id = envelope.request.request_id;

    if envelope.secret != endpoint.secret {
        return write_response(
            &mut write_half,
            ResponseEnvelope {
                request_id,
                decision: UserDecision::Deny,
                error: Some("unauthorized permission broker request".into()),
            },
        )
        .await;
    }

    let (decision_tx, decision_rx) = oneshot::channel();
    {
        let mut pending_requests = pending.lock().await;
        if pending_requests.contains_key(&request_id) {
            return write_response(
                &mut write_half,
                ResponseEnvelope {
                    request_id,
                    decision: UserDecision::Deny,
                    error: Some("duplicate permission request id".into()),
                },
            )
            .await;
        }
        pending_requests.insert(request_id, decision_tx);
    }

    if request_tx.send(envelope.request).await.is_err() {
        pending.lock().await.remove(&request_id);
        return write_response(
            &mut write_half,
            ResponseEnvelope {
                request_id,
                decision: UserDecision::Deny,
                error: Some("desktop permission receiver is unavailable".into()),
            },
        )
        .await;
    }

    let decision = match timeout(response_timeout, decision_rx).await {
        Ok(Ok(decision)) => decision,
        _ => UserDecision::Deny,
    };
    pending.lock().await.remove(&request_id);

    write_response(
        &mut write_half,
        ResponseEnvelope {
            request_id,
            decision,
            error: None,
        },
    )
    .await
}

async fn write_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    response: ResponseEnvelope,
) -> Result<(), BrokerError> {
    let mut payload = serde_json::to_vec(&response)?;
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(BrokerError::Rejected(
            "permission response exceeds broker message limit".into(),
        ));
    }
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}
