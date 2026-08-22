//! Update command handler.
//!
//! Fetches upstream commits for the whole forest, then advances one grove onto
//! them. The two halves remain available separately for when only one is wanted.

use crate::cli::args::UpdateArgs;
use crate::cli::commands::config::find_config;
use crate::cli::commands::grove::{render_update, resolve_grove};
use crate::domain::ForestRoot;
use crate::events::{ConsoleEmitter, EventEmitter};
use crate::hooks::HookRegistry;
use crate::ops::{GroveUpdateOperation, SyncSeedOperation, UpdateOptions};
use owo_colors::OwoColorize;
use std::sync::Arc;

pub fn run(args: UpdateArgs) -> miette::Result<()> {
    let grove_name = resolve_grove(args.name)?;
    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter: Arc<dyn EventEmitter> = Arc::new(ConsoleEmitter::new(args.verbose));
    let hooks = HookRegistry::from_config(&config.hook).map_err(|e| miette::miette!("{}", e))?;

    if args.dry_run {
        println!("{} dry run; nothing will be written", "!".yellow());
    }

    // Fetching first means the grove update that follows is purely local, and a
    // network failure leaves every checkout exactly as it was.
    if !args.dry_run {
        let sync =
            SyncSeedOperation::new(&forest_root, &config, Arc::clone(&emitter), args.verbose);
        sync.execute().map_err(|e| miette::miette!("{}", e))?;
    }

    let opts = UpdateOptions {
        strategy: args.strategy.into(),
        members: Vec::new(),
        dry_run: args.dry_run,
    };

    let op = GroveUpdateOperation::new(
        &forest_root,
        &config,
        &hooks,
        Arc::clone(&emitter),
        args.verbose,
    );
    let report = op
        .execute(&grove_name, &opts)
        .map_err(|e| miette::miette!("{}", e))?;

    render_update(&report);

    if report.has_failures() {
        std::process::exit(1);
    }

    Ok(())
}
