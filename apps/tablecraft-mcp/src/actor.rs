use crate::session::Session;
use serde_json::Value;
use std::{
    future::Future,
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
use tokio::sync::{mpsc, oneshot};

const QUEUED_CALLS: usize = 1;
const MAX_ARGUMENT_BYTES: usize = 8 * 1024 * 1024;

struct Request {
    operation: String,
    arguments: Value,
    reply: oneshot::Sender<Result<Value, String>>,
    cancelled: Arc<AtomicBool>,
}

struct CancelOnDrop {
    cancelled: Arc<AtomicBool>,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub(super) struct Actor {
    requests: mpsc::Sender<Request>,
}

impl Actor {
    pub(super) fn start(root: PathBuf) -> io::Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "root must be a directory",
            ));
        }

        let (requests, mut receiver) = mpsc::channel::<Request>(QUEUED_CALLS);
        thread::Builder::new()
            .name("tablecraft-mcp-session".into())
            .spawn(move || {
                let mut session = Session::new();
                while let Some(mut request) = receiver.blocking_recv() {
                    if request.cancelled.load(Ordering::Acquire) || request.reply.is_closed() {
                        continue;
                    }
                    let result = match &mut session {
                        Ok(session) => {
                            authorize_paths(&root, &mut request.arguments).and_then(|()| {
                                session.execute_cancellable(
                                    &request.operation,
                                    request.arguments,
                                    request.cancelled,
                                )
                            })
                        }
                        Err(error) => Err(error.to_string()),
                    };
                    // Losing a peer does not imply that a save was cancelled or rolled back.
                    // The session keeps its actual terminal report and recovery owner.
                    let _ = request.reply.send(result);
                }
            })?;
        Ok(Self { requests })
    }

    pub(super) async fn execute(
        &self,
        operation: String,
        arguments: Value,
        cancelled: impl Future<Output = ()>,
    ) -> Result<Value, String> {
        serde_json::to_writer(ArgumentBudget(MAX_ARGUMENT_BYTES), &arguments)
            .map_err(|_| "tool arguments exceed the host input budget".to_owned())?;
        let signal = Arc::new(AtomicBool::new(false));
        // NO_CLONE: The request actor must observe cancellation after this async call
        // ends or is dropped; the guard and actor share only one atomic signal.
        let _guard = CancelOnDrop {
            cancelled: Arc::clone(&signal),
        };
        let (reply, result) = oneshot::channel();
        let request = Request {
            operation,
            arguments,
            reply,
            cancelled: signal,
        };
        tokio::pin!(cancelled);
        tokio::select! {
            biased;
            () = &mut cancelled => {
                return Err("Tool request cancellation requested before queue acceptance".into());
            }
            accepted = self.requests.send(request) => {
                accepted.map_err(|_| "database session worker stopped".to_owned())?;
            }
        }
        tokio::select! {
            biased;
            () = &mut cancelled => Err("Tool request cancellation requested. An accepted native save or creation may still complete; use session_status to discover its retained operation ID".into()),
            response = result => response.map_err(|_| "database session worker stopped; save outcome may be uncertain".to_owned())?,
        }
    }
}

fn authorize_paths(root: &std::path::Path, arguments: &mut Value) -> Result<(), String> {
    for key in ["path", "bundle"] {
        let Some(value) = arguments.get(key).and_then(Value::as_str) else {
            continue;
        };
        let supplied = PathBuf::from(value);
        if !supplied.is_absolute() {
            return Err(format!(
                "{key} must be an absolute path within the configured root"
            ));
        }
        let resolved = if supplied.exists() {
            supplied.canonicalize()
        } else {
            let parent = supplied.parent().ok_or("path has no parent")?;
            let name = supplied.file_name().ok_or("path has no filename")?;
            parent.canonicalize().map(|parent| parent.join(name))
        }
        .map_err(|_| format!("cannot resolve {key} within the configured root"))?;
        if !resolved.starts_with(root) {
            return Err(format!("{key} is outside the configured root"));
        }
        let canonical = resolved
            .into_os_string()
            .into_string()
            .map_err(|_| "canonical paths must be representable as UTF-8".to_owned())?;
        arguments[key] = Value::String(canonical);
    }
    Ok(())
}

struct ArgumentBudget(usize);

impl io::Write for ArgumentBudget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.checked_sub(bytes.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "argument budget exceeded")
        })?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
