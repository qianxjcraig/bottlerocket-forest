//! Grove command handlers.

use crate::cli::args::{GroveCommand, GroveCreateArgs, GroveRemoveArgs, GroveUpdateArgs};
use crate::cli::commands::config::find_config;
use crate::domain::{ForestRoot, GroveName};
use crate::events::{ConsoleEmitter, EventEmitter};
use crate::grove::GroveContext;
use crate::hooks::HookRegistry;
use crate::ops::{
    GroveCreateOperation, GroveListOperation, GroveRemoveOperation, GroveUpdateOperation,
    MemberOutcome, UpdateOptions, UpdateReport,
};
use miette::Diagnostic;
use owo_colors::OwoColorize;
use snafu::{OptionExt, Snafu};
use std::sync::Arc;

#[derive(Debug, Snafu, Diagnostic)]
#[snafu(module)]
pub enum GroveError {
    #[snafu(display("cannot remove grove '{name}' while inside it"))]
    #[diagnostic(help("Change to a directory outside the grove before removing it"))]
    RemoveCurrentGrove { name: String },

    #[snafu(display("no grove named and not currently inside one"))]
    #[diagnostic(help("Name the grove explicitly, e.g. 'forester grove update develop'"))]
    NoGroveSelected,
}

pub fn run(cmd: GroveCommand) -> miette::Result<()> {
    match cmd {
        GroveCommand::Create(args) => create(args),
        GroveCommand::List => list(),
        GroveCommand::Remove(args) => remove(args),
        GroveCommand::Status => status_cmd(),
        GroveCommand::Current => current(),
        GroveCommand::Update(args) => update(args),
    }
}

fn update(args: GroveUpdateArgs) -> miette::Result<()> {
    let grove_name = resolve_grove(args.name)?;
    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter: Arc<dyn EventEmitter> = Arc::new(ConsoleEmitter::new(args.verbose));
    let hooks = HookRegistry::from_config(&config.hook).map_err(|e| miette::miette!("{}", e))?;

    let opts = UpdateOptions {
        strategy: args.strategy.into(),
        members: args.members,
        dry_run: args.dry_run,
    };

    if args.dry_run {
        println!("{} dry run; no checkout will be touched", "!".yellow());
    }

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

/// Resolves the grove to act on, defaulting to the current grove.
pub fn resolve_grove(name: Option<String>) -> miette::Result<GroveName> {
    use grove_error::*;

    let raw = match name {
        Some(name) => name,
        None => GroveContext::detect()
            .ok()
            .flatten()
            .map(|ctx| ctx.name().to_string())
            .context(NoGroveSelectedSnafu)?,
    };

    GroveName::try_new(raw).map_err(|e| miette::miette!("{}", e))
}

/// Prints the members an update declined to touch, each with its next step.
pub fn render_update(report: &UpdateReport) {
    let skipped: Vec<_> = report.skipped().collect();
    if skipped.is_empty() {
        return;
    }

    let width = skipped
        .iter()
        .map(|entry| entry.member.len())
        .max()
        .unwrap_or(0);

    println!();
    println!("{} left untouched:", "!".yellow());
    for entry in skipped {
        if let MemberOutcome::Skipped { reason } = &entry.outcome {
            println!(
                "  {:width$}  {}  {}",
                entry.member.cyan(),
                reason.describe(),
                format!("→ {}", reason.recovery_hint()).dimmed(),
                width = width
            );
        }
    }
}

fn create(args: GroveCreateArgs) -> miette::Result<()> {
    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter: Arc<dyn EventEmitter> = Arc::new(ConsoleEmitter::new(args.verbose));
    let hooks = HookRegistry::from_config(&config.hook).map_err(|e| miette::miette!("{}", e))?;

    let grove_name = GroveName::try_new(args.name).map_err(|e| miette::miette!("{}", e))?;

    let op = GroveCreateOperation::new(
        &forest_root,
        &config,
        &hooks,
        Arc::clone(&emitter),
        args.verbose,
    );
    op.execute(&grove_name, args.branch.as_deref())
        .map_err(|e| miette::miette!("{}", e))?;

    Ok(())
}

fn list() -> miette::Result<()> {
    let (forest_path, _config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter = ConsoleEmitter::new(false);
    let cwd = std::env::current_dir().ok();

    let op = GroveListOperation::new(&forest_root, &emitter, cwd);
    let groves = op.execute().map_err(|e| miette::miette!("{}", e))?;

    if groves.is_empty() {
        println!("No groves found");
    } else {
        println!("Forest groves:");
        for g in groves {
            if g.is_current {
                println!("* {} {}", g.name.to_string().cyan(), "(current)".dimmed());
            } else {
                println!("  {}", g.name.to_string().cyan());
            }
        }
    }

    Ok(())
}

fn remove(args: GroveRemoveArgs) -> miette::Result<()> {
    use grove_error::*;

    if let Ok(Some(ctx)) = GroveContext::detect()
        && ctx.name() == args.name
    {
        return Err(RemoveCurrentGroveSnafu { name: args.name }.build().into());
    }

    let (forest_path, config) = find_config()?;
    let forest_root = ForestRoot::builder().path(&forest_path).build();
    let emitter: Arc<dyn EventEmitter> = Arc::new(ConsoleEmitter::new(false));
    let hooks = HookRegistry::from_config(&config.hook).map_err(|e| miette::miette!("{}", e))?;

    let grove_name = GroveName::try_new(args.name).map_err(|e| miette::miette!("{}", e))?;

    let op = GroveRemoveOperation::new(&forest_root, &hooks, Arc::clone(&emitter), false);
    op.execute(&grove_name, args.force)
        .map_err(|e| miette::miette!("{}", e))?;

    Ok(())
}

fn status_cmd() -> miette::Result<()> {
    let (forest_path, _) = find_config()?;

    if let Some(ctx) = GroveContext::detect().ok().flatten() {
        println!("Grove:       {}", ctx.name().cyan());
        println!("Grove root:  {}", ctx.grove_root().display());
        println!("Forest root: {}", ctx.forest_root().display());
    } else {
        println!("Not in a grove");
        println!("Forest root: {}", forest_path.display());
    }

    Ok(())
}

fn current() -> miette::Result<()> {
    if let Some(ctx) = GroveContext::detect().ok().flatten() {
        println!("{}", ctx.name());
    } else {
        std::process::exit(1);
    }
    Ok(())
}
