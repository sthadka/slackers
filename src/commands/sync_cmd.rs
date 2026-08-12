use crate::cli::SyncCommand;
use crate::error::Result;

pub async fn handle_sync(_subcommand: SyncCommand, _read_only: bool) -> Result<()> {
    // Stub — full implementation provided by the store commands worker.
    todo!("sync command handler not yet implemented")
}
