//! CLI argument definitions.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Controls terminal color output behavior.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Parser)]
#[command(name = "forester")]
#[command(about = "Generic forest management for multi-repo projects")]
#[command(version)]
pub struct Cli {
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorChoice,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new forest in the current directory
    Init(InitArgs),
    /// Clone all member repositories and set up the forest
    Seed(SeedArgs),
    /// Fetch latest changes for all member repositories
    SyncSeed(SyncSeedArgs),
    /// Fetch upstream changes, then advance a grove onto them
    Update(UpdateArgs),
    /// Manage forest groves
    #[command(subcommand)]
    Grove(GroveCommand),
}

/// How far an update may go to bring a checkout current.
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum StrategyArg {
    /// Only advance branches with no local commits
    #[default]
    FfOnly,
    /// Replay local commits onto the new upstream base
    Rebase,
}

impl From<StrategyArg> for crate::ops::Strategy {
    fn from(arg: StrategyArg) -> Self {
        match arg {
            StrategyArg::FfOnly => Self::FfOnly,
            StrategyArg::Rebase => Self::Rebase,
        }
    }
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Grove to advance (defaults to the current grove)
    pub name: Option<String>,
    /// How far the update may go
    #[arg(short, long, value_enum, default_value_t = StrategyArg::FfOnly)]
    pub strategy: StrategyArg,
    /// Report what would change without writing anything
    #[arg(long)]
    pub dry_run: bool,
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Forest name
    #[arg(short, long)]
    pub name: Option<String>,
}

#[derive(Args, Debug)]
pub struct SyncSeedArgs {
    /// Path to forester.toml (default: current directory)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct SeedArgs {
    /// Path to forester.toml (default: current directory)
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum GroveCommand {
    /// Create a new grove
    Create(GroveCreateArgs),
    /// List all groves
    List,
    /// Remove a grove
    Remove(GroveRemoveArgs),
    /// Show current grove status
    Status,
    /// Print current grove name
    Current,
    /// Advance a grove's checkouts onto the last fetched commits
    Update(GroveUpdateArgs),
}

#[derive(Args, Debug)]
pub struct GroveUpdateArgs {
    /// Grove to update (defaults to the current grove)
    pub name: Option<String>,
    /// How far the update may go
    #[arg(short, long, value_enum, default_value_t = StrategyArg::FfOnly)]
    pub strategy: StrategyArg,
    /// Restrict the update to specific members (repeatable)
    #[arg(short, long = "member")]
    pub members: Vec<String>,
    /// Report intended actions without touching any checkout
    #[arg(long)]
    pub dry_run: bool,
    /// Show verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct GroveCreateArgs {
    pub name: String,
    #[arg(short, long)]
    pub branch: Option<String>,
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Args, Debug)]
pub struct GroveRemoveArgs {
    pub name: String,
    #[arg(short, long)]
    pub force: bool,
}
