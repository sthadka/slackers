use crate::cli::QueryCommand;
use crate::error::Result;
use crate::store::query::QueryFilters;

pub async fn handle_query(cmd: QueryCommand) -> Result<()> {
    let resolved = crate::auth::resolve_auth(None)?;
    let workspace_url = resolved.workspace_url.unwrap_or_default();
    let db_path = crate::config::store_db_path(&workspace_url)?;
    let store = crate::store::Store::open(&db_path)?;

    let results = match cmd {
        QueryCommand::Messages(opts) => {
            let filters = QueryFilters {
                user: opts.user,
                channel: opts.channel,
                after: opts.after,
                before: opts.before,
                text: opts.text,
                sort: opts.sort,
                limit: opts.limit,
                ..Default::default()
            };
            store.query_messages(&filters)?
        }
        QueryCommand::Threads(opts) => {
            let filters = QueryFilters {
                user: opts.user,
                channel: opts.channel,
                after: opts.after,
                before: opts.before,
                sort: opts.sort,
                limit: opts.limit,
                ..Default::default()
            };
            store.query_threads(&filters)?
        }
        QueryCommand::Reactions(opts) => {
            let filters = QueryFilters {
                channel: opts.channel,
                user: opts.user,
                emoji: opts.emoji,
                group_by: opts.group_by,
                limit: opts.limit,
                ..Default::default()
            };
            store.query_reactions(&filters)?
        }
        QueryCommand::Files(opts) => {
            let filters = QueryFilters {
                channel: opts.channel,
                text: opts.text,
                sort: opts.sort,
                limit: opts.limit,
                ..Default::default()
            };
            store.query_files(&filters)?
        }
        QueryCommand::Activity(opts) => {
            let filters = QueryFilters {
                channel: opts.channel,
                user: opts.user,
                after: opts.after,
                before: opts.before,
                limit: opts.limit,
                ..Default::default()
            };
            store.query_activity(&filters)?
        }
    };

    println!("{}", crate::output::to_json_output(&results));
    Ok(())
}
