//! Sync-seed command handler.

use crate::cli::args::SyncSeedArgs;
use crate::cli::commands::config::resolve_forest;
use crate::domain::ForestRoot;
use crate::events::{ConsoleEmitter, EventEmitter};
use crate::ops::SyncSeedOperation;
use std::sync::Arc;

pub fn run(args: SyncSeedArgs) -> miette::Result<()> {
    let (forest_path, config) = resolve_forest(args.config)?;

    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter: Arc<dyn EventEmitter> = Arc::new(ConsoleEmitter::new(args.verbose));

    let op = SyncSeedOperation::new(&forest_root, &config, Arc::clone(&emitter), args.verbose);
    op.execute().map_err(|e| miette::miette!("{}", e))?;

    Ok(())
}
