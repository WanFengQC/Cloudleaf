use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt;
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u32 = 1;
pub const RESULT_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 128 * 1024;
pub const MAX_INPUT_BYTES: usize = 16 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024;
pub const MAX_PENDING: usize = 2;
pub const MAX_TIMEOUT_MS: u64 = 120_000;
pub const PROVIDER_ID: &str = "offline-model";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    ZhEn,
    EnZh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub session: u64,
    pub epoch: u64,
    pub input_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestKey {
    pub generation: u64,
    pub source: SourceIdentity,
    pub request_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceThreads {
    One,
    Two,
    Four,
}

impl ResourceThreads {
    pub const fn count(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

impl TryFrom<usize> for ResourceThreads {
    type Error = IpcError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            4 => Ok(Self::Four),
            _ => Err(IpcError::InvalidConfiguration),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcError {
    InvalidConfiguration,
    InvalidInput,
    InputTooLong,
    Timeout,
    Busy,
    Cancelled,
    InvalidWorkerPath,
    WorkerStartFailed,
    WorkerExited,
    BrokenPipe,
    InvalidProtocolVersion,
    FrameTooLarge,
    TruncatedFrame,
    InvalidUtf8,
    InvalidMessage,
    InvalidIdentity,
    DuplicateRequest,
    ModelNotFound,
    ModelInvalid,
    ModelLoadFailed,
    UnsupportedDirection,
    InvalidResponse,
    OutputTruncated,
    Internal,
}

impl fmt::Display for IpcError {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(writer, "{self:?}")
    }
}

impl std::error::Error for IpcError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStage {
    Verifying,
    Loading,
    Inferring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    ResearchOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerMetrics {
    pub compute_threads: usize,
    pub resident_models: usize,
    pub verification_started_unix_ns: u64,
    pub loading_started_unix_ns: u64,
    pub inference_started_unix_ns: u64,
    pub inference_finished_unix_ns: u64,
    pub verification_ns: u64,
    pub load_ns: u64,
    pub inference_ns: u64,
    pub encoder_ns: u64,
    pub decoder_ns: u64,
    pub generated_steps: usize,
    pub queue_wait_ns: u64,
    pub output_truncated: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerOutput {
    pub raw_translation: String,
    pub direction: Direction,
    pub provider_identity: String,
    pub model_identity: String,
    pub catalog_sha256: String,
    pub quality: QualityStatus,
    pub metrics: Box<WorkerMetrics>,
}

impl fmt::Debug for WorkerOutput {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        writer
            .debug_struct("WorkerOutput")
            .field("translation_bytes", &self.raw_translation.len())
            .field("direction", &self.direction)
            .field("provider_identity", &self.provider_identity)
            .field("model_identity", &self.model_identity)
            .field("catalog_sha256", &self.catalog_sha256)
            .field("quality", &self.quality)
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<Message> {
    pub protocol_version: u32,
    pub worker_generation: u64,
    pub message: Message,
}

impl<Message> Envelope<Message> {
    pub fn new(worker_generation: u64, message: Message) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            worker_generation,
            message,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Handshake {
        catalog_sha256: String,
    },
    Request {
        key: RequestKey,
        direction: Direction,
        text: String,
        timeout_ms: u64,
    },
    Cancel {
        key: RequestKey,
    },
    Shutdown {},
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Capabilities {
        provider_identity: String,
        catalog_sha256: String,
        threads: ResourceThreads,
        max_pending: usize,
    },
    Accepted {
        key: RequestKey,
        task_id: u64,
    },
    Progress {
        key: RequestKey,
        task_id: u64,
        stage: ProgressStage,
    },
    Result {
        key: RequestKey,
        task_id: u64,
        result_version: u32,
        output: WorkerOutput,
    },
    Cancelled {
        key: RequestKey,
    },
    Error {
        key: Option<RequestKey>,
        error: IpcError,
    },
}

pub fn validate_text(text: &str, byte_limit: usize) -> Result<(), IpcError> {
    if text.len() > byte_limit {
        return Err(IpcError::InputTooLong);
    }
    if text.trim().is_empty() || text.contains('\0') {
        return Err(IpcError::InvalidInput);
    }
    Ok(())
}

pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn read_frame<T: DeserializeOwned>(mut input: impl Read) -> Result<Option<T>, IpcError> {
    let mut header = [0; 4];
    loop {
        match input.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(read_error(error)),
        }
    }
    input.read_exact(&mut header[1..]).map_err(read_error)?;
    let size = u32::from_be_bytes(header) as usize;
    if size > MAX_FRAME_BYTES {
        return Err(IpcError::FrameTooLarge);
    }
    if size == 0 {
        return Err(IpcError::InvalidMessage);
    }
    let mut body = vec![0; size];
    input.read_exact(&mut body).map_err(read_error)?;
    let body = std::str::from_utf8(&body).map_err(|_| IpcError::InvalidUtf8)?;
    serde_json::from_str(body)
        .map(Some)
        .map_err(|_| IpcError::InvalidMessage)
}

fn read_error(error: io::Error) -> IpcError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        IpcError::TruncatedFrame
    } else {
        IpcError::BrokenPipe
    }
}

pub fn write_frame<T: Serialize>(mut output: impl Write, message: &T) -> Result<(), IpcError> {
    let body = serde_json::to_vec(message).map_err(|_| IpcError::InvalidMessage)?;
    if body.len() > MAX_FRAME_BYTES {
        return Err(IpcError::FrameTooLarge);
    }
    output
        .write_all(&(body.len() as u32).to_be_bytes())
        .and_then(|()| output.write_all(&body))
        .and_then(|()| output.flush())
        .map_err(|_| IpcError::BrokenPipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_boundaries_do_not_deserialize_invalid_input() {
        let frame = |data: &[u8]| read_frame::<Envelope<Command>>(data).err();
        assert_eq!(frame(&[]), None);
        assert_eq!(frame(&[0]), Some(IpcError::TruncatedFrame));
        assert_eq!(frame(&[0, 0, 0, 2, 1]), Some(IpcError::TruncatedFrame));
        assert_eq!(frame(&[0, 0, 0, 1, 0xff]), Some(IpcError::InvalidUtf8));
        assert_eq!(frame(&[0, 0, 0, 0]), Some(IpcError::InvalidMessage));
        assert_eq!(
            frame(&((MAX_FRAME_BYTES + 1) as u32).to_be_bytes()),
            Some(IpcError::FrameTooLarge)
        );
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &Envelope::new(8, Command::Shutdown {})).unwrap();
        let parsed = read_frame::<Envelope<Command>>(&bytes[..])
            .unwrap()
            .unwrap();
        assert_eq!(parsed.worker_generation, 8);
        assert!(matches!(parsed.message, Command::Shutdown {}));
    }

    #[test]
    fn unknown_commands_and_extra_fields_are_rejected() {
        let mut bytes = Vec::new();
        for message in [
            serde_json::json!({"kind": "future"}),
            serde_json::json!({"kind": "shutdown", "unexpected": true}),
        ] {
            bytes.clear();
            write_frame(
                &mut bytes,
                &serde_json::json!({"protocol_version": 1, "worker_generation": 1, "message": message}),
            )
            .unwrap();
            assert!(matches!(
                read_frame::<Envelope<Command>>(&bytes[..]),
                Err(IpcError::InvalidMessage)
            ));
        }
    }
}
