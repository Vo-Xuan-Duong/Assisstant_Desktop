use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use tracing_subscriber::EnvFilter;

const APP_IDENTIFIER: &str = "com.voduong.assisstantdesktop";
const LOG_DIR_ENV: &str = "ASSISTANT_LOG_DIR";
const LOG_FILE_NAME: &str = "assistant.log";
const LOG_BACKUP_NAME: &str = "assistant.log.1";
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

/// Install the process-wide tracing subscriber before the Tauri runtime starts.
///
/// The packaged desktop process is a background application, so stderr alone is
/// not a useful operational surface. This writer keeps stderr for local/dev use
/// and mirrors the same formatted records into a bounded per-user log file.
pub fn init() {
    let filter = || {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let persistent = resolve_log_path().and_then(RotatingLogWriter::open);
    match persistent {
        Ok(file_writer) => {
            let path = file_writer.path().to_path_buf();
            let make_writer = file_writer.clone();
            let initialized = tracing_subscriber::fmt()
                .with_env_filter(filter())
                .with_ansi(false)
                .with_writer(move || TeeLogWriter {
                    stderr: io::stderr(),
                    file: make_writer.clone(),
                })
                .try_init()
                .is_ok();

            if initialized {
                tracing::info!(path = %path.display(), max_bytes = MAX_LOG_BYTES, "persistent runtime logging initialized");
            }
        }
        Err(error) => {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter())
                .with_ansi(false)
                .with_writer(io::stderr)
                .try_init();
            tracing::warn!(%error, "persistent runtime log is unavailable; using stderr only");
        }
    }
}

fn resolve_log_path() -> io::Result<PathBuf> {
    if let Some(value) = env::var_os(LOG_DIR_ENV) {
        let directory = PathBuf::from(value);
        if !directory.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{LOG_DIR_ENV} must be an absolute path"),
            ));
        }
        return Ok(directory.join(LOG_FILE_NAME));
    }

    #[cfg(windows)]
    {
        let local = env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable")
        })?;
        return Ok(PathBuf::from(local)
            .join(APP_IDENTIFIER)
            .join("logs")
            .join(LOG_FILE_NAME));
    }

    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is unavailable"))?;
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(APP_IDENTIFIER)
            .join("logs")
            .join(LOG_FILE_NAME))
    }
}

#[derive(Clone)]
struct RotatingLogWriter {
    inner: Arc<Mutex<RotatingLogState>>,
    path: Arc<PathBuf>,
}

impl RotatingLogWriter {
    fn open(path: PathBuf) -> io::Result<Self> {
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "runtime log path has no parent")
        })?;
        fs::create_dir_all(parent)?;

        let backup = parent.join(LOG_BACKUP_NAME);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let mut state = RotatingLogState {
            file: Some(file),
            bytes,
            path: path.clone(),
            backup,
        };
        if state.bytes >= MAX_LOG_BYTES {
            state.rotate()?;
        }

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
            path: Arc::new(path),
        })
    }

    fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl Write for RotatingLogWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("runtime log mutex is poisoned"))?;
        state.write_record(buffer)?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("runtime log mutex is poisoned"))?;
        state.flush()
    }
}

struct RotatingLogState {
    file: Option<File>,
    bytes: u64,
    path: PathBuf,
    backup: PathBuf,
}

impl RotatingLogState {
    fn write_record(&mut self, buffer: &[u8]) -> io::Result<()> {
        let incoming = u64::try_from(buffer.len()).unwrap_or(u64::MAX);
        if self.bytes > 0 && self.bytes.saturating_add(incoming) > MAX_LOG_BYTES {
            self.rotate()?;
        }

        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("runtime log file is not open"))?;
        file.write_all(buffer)?;
        self.bytes = self.bytes.saturating_add(incoming);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
            let _ = file.sync_data();
        }

        if self.backup.exists() {
            fs::remove_file(&self.backup)?;
        }

        if self.path.exists() {
            if let Err(error) = fs::copy(&self.path, &self.backup) {
                self.reopen_append()?;
                return Err(error);
            }
        }

        self.file = Some(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.path)?,
        );
        self.bytes = 0;
        Ok(())
    }

    fn reopen_append(&mut self) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(self.bytes);
        self.file = Some(file);
        Ok(())
    }
}

struct TeeLogWriter {
    stderr: io::Stderr,
    file: RotatingLogWriter,
}

impl Write for TeeLogWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.file.write_all(buffer)?;
        let _ = self.stderr.write_all(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()?;
        let _ = self.stderr.flush();
        Ok(())
    }
}
