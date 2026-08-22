//! Rich terminal output with progress bars.

use std::sync::Mutex;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use super::{EventEmitter, ForesterEvent};

/// Event emitter with indicatif progress bar support.
#[derive(Debug)]
pub struct RichEmitter {
    mp: MultiProgress,
    current_bar: Mutex<Option<ProgressBar>>,
}

impl RichEmitter {
    /// Creates a new rich emitter.
    pub fn new() -> Self {
        Self {
            mp: MultiProgress::new(),
            current_bar: Mutex::new(None),
        }
    }

    fn spinner(&self, msg: &str) -> ProgressBar {
        let pb = self.mp.add(ProgressBar::new_spinner());
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb
    }

    #[allow(clippy::expect_used)]
    fn lock_bar(&self) -> std::sync::MutexGuard<'_, Option<ProgressBar>> {
        self.current_bar.lock().expect("lock poisoned")
    }

    fn finish_current(&self) {
        if let Some(pb) = self.lock_bar().take() {
            pb.finish_and_clear();
        }
    }
}

impl Default for RichEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter for RichEmitter {
    fn emit(&self, event: &ForesterEvent) {
        match event {
            ForesterEvent::SeedStarted => {
                let pb = self.spinner("Seeding forest...");
                *self.lock_bar() = Some(pb);
            }
            ForesterEvent::MemberCloning { name, .. } => {
                if let Some(pb) = self.lock_bar().as_ref() {
                    pb.set_message(format!("Cloning {name}..."));
                }
            }
            ForesterEvent::MemberCloned { .. } => {}
            ForesterEvent::SeedCompleted => {
                self.finish_current();
                eprintln!("✓ Forest seeded");
            }
            ForesterEvent::SyncSeedStarted => {
                let pb = self.spinner("Syncing forest...");
                *self.lock_bar() = Some(pb);
            }
            ForesterEvent::MemberFetching { name } => {
                if let Some(pb) = self.lock_bar().as_ref() {
                    pb.set_message(format!("Fetching {name}..."));
                }
            }
            ForesterEvent::MemberFetched { .. } => {}
            ForesterEvent::MemberSkipped { name, reason } => {
                self.mp.suspend(|| eprintln!("⚠ {name} skipped: {reason}"));
            }
            ForesterEvent::GroveUpdateStarted { grove } => {
                let bar = self.spinner(&format!("Updating grove '{}'", grove));
                *self.lock_bar() = Some(bar);
            }
            ForesterEvent::MemberUpdating { name } => {
                if let Some(bar) = self.lock_bar().as_ref() {
                    bar.set_message(format!("Updating {}", name));
                }
            }
            ForesterEvent::MemberUpdated { name, detail } => {
                self.finish_current();
                eprintln!("  ↑ {name} ({detail})");
            }
            ForesterEvent::MemberCurrent { .. } => {}
            ForesterEvent::GroveUpdateCompleted {
                grove,
                updated,
                skipped,
            } => {
                self.finish_current();
                eprintln!("✓ Grove '{grove}' updated ({updated} advanced, {skipped} untouched)");
            }
            ForesterEvent::SyncSeedCompleted => {
                self.finish_current();
                eprintln!("✓ Forest synced");
            }
            ForesterEvent::GroveCreating { name } => {
                let pb = self.spinner(&format!("Creating grove '{name}'..."));
                *self.lock_bar() = Some(pb);
            }
            ForesterEvent::GroveWorktreeCreated { .. } => {}
            ForesterEvent::GroveCreated { name } => {
                self.finish_current();
                eprintln!("✓ Grove '{name}' created");
            }
            ForesterEvent::GroveRemoving { name } => {
                let pb = self.spinner(&format!("Removing grove '{name}'..."));
                *self.lock_bar() = Some(pb);
            }
            ForesterEvent::GroveRemoved { name } => {
                self.finish_current();
                eprintln!("✓ Grove '{name}' removed");
            }
            ForesterEvent::GroveListing => {}
            ForesterEvent::HookExecuting { name } => {
                if let Some(pb) = self.lock_bar().as_ref() {
                    pb.set_message(format!("Running hook '{name}'..."));
                }
            }
            ForesterEvent::HookCompleted { .. } => {}
            ForesterEvent::Warning(msg) => {
                self.mp.suspend(|| eprintln!("⚠ {msg}"));
            }
            ForesterEvent::Info(msg) => {
                self.mp.suspend(|| eprintln!("ℹ {msg}"));
            }
            ForesterEvent::Error(msg) => {
                self.mp.suspend(|| eprintln!("✗ {msg}"));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn constructs_with_new() {
        // Given/When we create a RichEmitter
        let emitter = RichEmitter::new();

        // Then it should exist (no panic)
        drop(emitter);
    }

    #[test]
    fn constructs_with_default() {
        // Given/When we create a RichEmitter via Default
        let emitter = RichEmitter::default();

        // Then it should exist (no panic)
        drop(emitter);
    }

    #[test]
    fn emits_seed_started() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit SeedStarted
        emitter.emit(&ForesterEvent::SeedStarted);

        // Then no panic occurs
    }

    #[test]
    fn emits_member_cloning() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit MemberCloning
        emitter.emit(&ForesterEvent::MemberCloning {
            name: "test-repo".to_string(),
            remote: "https://example.com/repo.git".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_member_cloned() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit MemberCloned
        emitter.emit(&ForesterEvent::MemberCloned {
            name: "test-repo".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_seed_completed() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit SeedCompleted
        emitter.emit(&ForesterEvent::SeedCompleted);

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_creating() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveCreating
        emitter.emit(&ForesterEvent::GroveCreating {
            name: "test-grove".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_worktree_created() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveWorktreeCreated
        emitter.emit(&ForesterEvent::GroveWorktreeCreated {
            member: "test-member".to_string(),
            path: PathBuf::from("/tmp/test"),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_created() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveCreated
        emitter.emit(&ForesterEvent::GroveCreated {
            name: "test-grove".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_removing() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveRemoving
        emitter.emit(&ForesterEvent::GroveRemoving {
            name: "test-grove".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_removed() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveRemoved
        emitter.emit(&ForesterEvent::GroveRemoved {
            name: "test-grove".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_grove_listing() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit GroveListing
        emitter.emit(&ForesterEvent::GroveListing);

        // Then no panic occurs
    }

    #[test]
    fn emits_hook_executing() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit HookExecuting
        emitter.emit(&ForesterEvent::HookExecuting {
            name: "test-hook".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_hook_completed() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit HookCompleted
        emitter.emit(&ForesterEvent::HookCompleted {
            name: "test-hook".to_string(),
        });

        // Then no panic occurs
    }

    #[test]
    fn emits_warning() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit Warning
        emitter.emit(&ForesterEvent::Warning("test warning".to_string()));

        // Then no panic occurs (styled output produced)
    }

    #[test]
    fn emits_info() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit Info
        emitter.emit(&ForesterEvent::Info("test info".to_string()));

        // Then no panic occurs (styled output produced)
    }

    #[test]
    fn emits_error() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit Error
        emitter.emit(&ForesterEvent::Error("test error".to_string()));

        // Then no panic occurs (styled output produced)
    }

    #[test]
    fn handles_full_seed_workflow() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit a full seed workflow sequence
        emitter.emit(&ForesterEvent::SeedStarted);
        emitter.emit(&ForesterEvent::MemberCloning {
            name: "repo1".to_string(),
            remote: "https://example.com/repo1.git".to_string(),
        });
        emitter.emit(&ForesterEvent::MemberCloned {
            name: "repo1".to_string(),
        });
        emitter.emit(&ForesterEvent::SeedCompleted);

        // Then no panic occurs
    }

    #[test]
    fn handles_full_grove_workflow() {
        // Given a RichEmitter
        let emitter = RichEmitter::new();

        // When we emit a full grove creation workflow
        emitter.emit(&ForesterEvent::GroveCreating {
            name: "my-grove".to_string(),
        });
        emitter.emit(&ForesterEvent::GroveWorktreeCreated {
            member: "repo1".to_string(),
            path: PathBuf::from("/tmp/grove/repo1"),
        });
        emitter.emit(&ForesterEvent::HookExecuting {
            name: "post-create".to_string(),
        });
        emitter.emit(&ForesterEvent::HookCompleted {
            name: "post-create".to_string(),
        });
        emitter.emit(&ForesterEvent::GroveCreated {
            name: "my-grove".to_string(),
        });

        // Then no panic occurs
    }
}
