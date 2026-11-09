//! Explicit local stdio host. Database work runs on a dedicated session thread.
mod actor;
mod permissions;
mod protocol;
mod requests;
mod session;
mod tools;

use clap::Parser;
use permissions::Permissions;
use rmcp::{ServiceExt, transport::stdio};
use std::{error::Error, path::PathBuf};

#[derive(Parser)]
#[command(
    version,
    about = "Local TableCraft MCP server over stdin/stdout; read-only by default"
)]
struct Args {
    /// Directory containing the databases and save bundles this process may access.
    #[arg(long)]
    root: PathBuf,
    /// Allow applying reviewed row changes and INSERT/UPDATE/DELETE queries.
    #[arg(long)]
    allow_data_write: bool,
    /// Allow database creation and applying reviewed table, field or database-option changes.
    #[arg(long)]
    allow_schema_write: bool,
    /// Allow applying reviewed saved-query SQL/expression changes.
    #[arg(long)]
    allow_application_write: bool,
    /// Allow starting checked reserved-journal recovery, which may restore data or schema.
    #[arg(long)]
    allow_maintenance: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let permissions = Permissions {
        data_write: args.allow_data_write,
        schema_write: args.allow_schema_write,
        application_write: args.allow_application_write,
        maintenance: args.allow_maintenance,
    };
    let actor = actor::Actor::start(args.root, permissions)?;
    let service = protocol::Server::new(actor).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
