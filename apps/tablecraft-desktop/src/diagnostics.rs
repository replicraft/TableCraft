use std::io;
use tracing::Level;
use tracing_subscriber::fmt::{format::FmtSpan, time::uptime};

pub(crate) fn init(verbose: u8) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let level = match verbose {
        0 => return Ok(()),
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(io::stderr)
        .with_ansi(false)
        .with_timer(uptime())
        .with_span_events(FmtSpan::CLOSE)
        .try_init()
}
