//! Event emission for forester operations.

use owo_colors::OwoColorize;
use std::io::IsTerminal;

use super::ForesterEvent;

/// Creates an event emitter appropriate for the current environment.
pub fn create_emitter(verbose: bool) -> std::sync::Arc<dyn EventEmitter + Send + Sync> {
    if std::io::stdout().is_terminal() {
        std::sync::Arc::new(super::RichEmitter::new())
    } else {
        std::sync::Arc::new(ConsoleEmitter::new(verbose))
    }
}

/// Trait for emitting forester events.
pub trait EventEmitter {
    /// Emit an event.
    fn emit(&self, event: &ForesterEvent);
}

/// Console-based event emitter with color support.
#[derive(Debug, Clone)]
pub struct ConsoleEmitter {
    verbose: bool,
}

impl ConsoleEmitter {
    /// Creates a new console emitter.
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl EventEmitter for ConsoleEmitter {
    fn emit(&self, event: &ForesterEvent) {
        match event {
            ForesterEvent::SeedStarted => {
                eprintln!("{} Seeding forest...", "→".cyan());
            }
            ForesterEvent::MemberCloning { name, remote } => {
                if self.verbose {
                    eprintln!("  {} {} from {}", "cloning".blue(), name, remote.dimmed());
                } else {
                    eprintln!("  {} {}", "cloning".blue(), name);
                }
            }
            ForesterEvent::MemberCloned { name } => {
                if self.verbose {
                    eprintln!("  {} {}", "cloned".green(), name);
                }
            }
            ForesterEvent::SeedCompleted => {
                eprintln!("{} Forest seeded", "✓".green());
            }
            ForesterEvent::SyncSeedStarted => {
                eprintln!("{} Syncing forest...", "→".cyan());
            }
            ForesterEvent::MemberFetching { name } => {
                eprintln!("  {} {}", "fetching".blue(), name);
            }
            ForesterEvent::MemberFetched { name } => {
                if self.verbose {
                    eprintln!("  {} {}", "fetched".green(), name);
                }
            }
            ForesterEvent::MemberSkipped { name, reason } => {
                eprintln!("{} {} ({})", "!".yellow(), name, reason);
            }
            ForesterEvent::SyncSeedCompleted => {
                eprintln!("{} Forest synced", "✓".green());
            }
            ForesterEvent::GroveUpdateStarted { grove } => {
                eprintln!("{} Updating grove '{}'", "→".cyan(), grove);
            }
            ForesterEvent::MemberUpdating { name } => {
                if self.verbose {
                    eprintln!("  {} {}", "updating".blue(), name);
                }
            }
            ForesterEvent::MemberUpdated { name, detail } => {
                eprintln!("  {} {} ({})", "↑".green(), name, detail);
            }
            ForesterEvent::MemberCurrent { name } => {
                if self.verbose {
                    eprintln!("  {} {}", "current".green(), name);
                }
            }
            ForesterEvent::GroveUpdateCompleted {
                grove,
                updated,
                skipped,
            } => {
                if *skipped > 0 {
                    eprintln!(
                        "{} Grove '{}' updated ({} advanced, {} left untouched)",
                        "✓".green(),
                        grove,
                        updated,
                        skipped
                    );
                } else {
                    eprintln!(
                        "{} Grove '{}' updated ({} advanced)",
                        "✓".green(),
                        grove,
                        updated
                    );
                }
            }
            ForesterEvent::GroveCreating { name } => {
                eprintln!("{} Creating grove '{}'", "→".cyan(), name);
            }
            ForesterEvent::GroveWorktreeCreated { member, path } => {
                if self.verbose {
                    eprintln!(
                        "  {} {} at {}",
                        "worktree".blue(),
                        member,
                        path.display().to_string().dimmed()
                    );
                }
            }
            ForesterEvent::GroveCreated { name } => {
                eprintln!("{} Grove '{}' created", "✓".green(), name);
            }
            ForesterEvent::GroveRemoving { name } => {
                eprintln!("{} Removing grove '{}'", "→".cyan(), name);
            }
            ForesterEvent::GroveRemoved { name } => {
                eprintln!("{} Grove '{}' removed", "✓".green(), name);
            }
            ForesterEvent::GroveListing => {
                if self.verbose {
                    eprintln!("{} Listing groves", "→".cyan());
                }
            }
            ForesterEvent::HookExecuting { name } => {
                eprintln!("  {} hook '{}'", "running".blue(), name);
            }
            ForesterEvent::HookCompleted { name } => {
                if self.verbose {
                    eprintln!("  {} hook '{}'", "completed".green(), name);
                }
            }
            ForesterEvent::Warning(msg) => {
                eprintln!("{} {}", "warning:".yellow(), msg);
            }
            ForesterEvent::Info(msg) => {
                eprintln!("{} {}", "info:".blue(), msg);
            }
            ForesterEvent::Error(msg) => {
                eprintln!("{} {}", "error:".red(), msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_event_formats_with_red_prefix() {
        let emitter = ConsoleEmitter::new(false);
        let event = ForesterEvent::Error("test message".to_string());
        // Verify the emitter can be called without panic.
        // The actual output goes to stderr with red "error:" prefix.
        emitter.emit(&event);
    }

    #[test]
    fn create_emitter_can_emit_events_without_panic() {
        // Given an emitter from the factory
        let emitter = create_emitter(false);

        // When emitting an event
        let event = ForesterEvent::Info("test".to_string());

        // Then it should not panic
        emitter.emit(&event);
    }
}
