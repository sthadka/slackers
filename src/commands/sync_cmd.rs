use crate::cli::SyncCommand;
use crate::error::Result;

pub async fn handle_sync(cmd: SyncCommand, _read_only: bool) -> Result<()> {
    match cmd {
        SyncCommand::Start(_opts) => {
            eprintln!("Not yet implemented. Real-time sync will be available in a future release.");
            eprintln!("Use `slackers sync backfill` to do a one-time historical sync.");
            Ok(())
        }
        SyncCommand::Stop => {
            eprintln!("Not yet implemented. Sync daemon stop will be available in a future release.");
            Ok(())
        }
        SyncCommand::Status => {
            eprintln!("Not yet implemented. Use `slackers store info` to see current sync state.");
            Ok(())
        }
        SyncCommand::Backfill(_opts) => {
            eprintln!("Not yet implemented. Run `slackers sync backfill` after subscribing to channels.");
            eprintln!("Subscribe first with: slackers store sub add #channel-name");
            Ok(())
        }
        SyncCommand::Once => {
            eprintln!("Not yet implemented. One-shot incremental sync will be available in a future release.");
            Ok(())
        }
    }
}
