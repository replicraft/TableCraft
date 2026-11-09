//! Explicit local stdio host. Database work runs on a dedicated session thread.
mod actor;
mod protocol;
mod requests;
mod session;
mod tools;

use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use std::{error::Error, path::PathBuf};

#[derive(Parser)]
#[command(version, about = "Local TableCraft MCP server over stdin/stdout")]
struct Args {
    /// Directory containing the databases and save bundles this process may access.
    #[arg(long)]
    root: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let actor = actor::Actor::start(args.root)?;
    let service = protocol::Server::new(actor).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
