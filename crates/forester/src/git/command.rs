//! Git command execution with proper error capture.

use miette::Diagnostic;
use snafu::{ResultExt, Snafu};
use std::path::PathBuf;
use std::process::Command;

/// Error from git command execution.
#[derive(Debug, Snafu, Diagnostic)]
#[snafu(module)]
pub enum GitCommandError {
    /// Git command failed.
    #[snafu(display("git {command} failed (exit code: {exit_code:?})"))]
    #[diagnostic(help("{stderr}"))]
    CommandFailed {
        /// The git subcommand that failed.
        command: String,
        /// The exit code from git.
        exit_code: Option<i32>,
        /// The stderr output from git.
        stderr: String,
    },

    /// Failed to execute git.
    #[snafu(display("Failed to execute git"))]
    Io {
        /// The underlying IO error.
        source: std::io::Error,
    },

    /// Git succeeded but produced output that could not be interpreted.
    #[snafu(display("Could not interpret output of 'git {command}'"))]
    #[diagnostic(help("{detail}"))]
    UnexpectedOutput {
        /// The git subcommand that ran.
        command: String,
        /// What could not be interpreted.
        detail: String,
    },
}

/// Builder for git commands that always captures stderr.
pub struct GitCommand {
    args: Vec<String>,
    cwd: Option<PathBuf>,
    quiet_stdout: bool,
}

impl GitCommand {
    /// Creates a new git command with the given subcommand.
    pub fn new(subcommand: &str) -> Self {
        Self {
            args: vec![subcommand.to_string()],
            cwd: None,
            quiet_stdout: false,
        }
    }

    /// Adds an argument.
    pub fn arg(mut self, arg: impl AsRef<str>) -> Self {
        self.args.push(arg.as_ref().to_string());
        self
    }

    /// Adds multiple arguments.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.args
            .extend(args.into_iter().map(|s| s.as_ref().to_string()));
        self
    }

    /// Sets the working directory.
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Sets whether to suppress stdout (stderr is always captured).
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet_stdout = quiet;
        self
    }

    /// Executes the command.
    pub fn run(self) -> Result<(), GitCommandError> {
        let quiet = self.quiet_stdout;
        let outcome = self.run_output()?;

        if !quiet && !outcome.stdout.is_empty() {
            println!("{}", outcome.stdout);
        }

        outcome.into_success().map(|_| ())
    }

    /// Executes the command and returns its stdout, failing on a nonzero exit.
    pub fn run_capture(self) -> Result<String, GitCommandError> {
        self.run_output()?.into_success()
    }

    /// Executes the command, treating a nonzero exit as data rather than failure.
    ///
    /// Use for probes such as `rev-parse --verify`, where "this ref does not
    /// exist" is an answer and not an error.
    pub fn run_output(self) -> Result<GitOutcome, GitCommandError> {
        use git_command_error::*;

        let mut cmd = Command::new("git");
        cmd.args(&self.args);

        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }

        let output = cmd.output().context(IoSnafu)?;

        Ok(GitOutcome {
            command: self.args.first().cloned().unwrap_or_default(),
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Captured result of a git invocation.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GitOutcome {
    /// The git subcommand that ran.
    pub command: String,
    /// Whether git exited successfully.
    pub success: bool,
    /// Exit code reported by git.
    pub exit_code: Option<i32>,
    /// Trimmed stdout.
    pub stdout: String,
    /// Trimmed stderr.
    pub stderr: String,
}

impl GitOutcome {
    /// Returns stdout, converting a nonzero exit into an error.
    pub fn into_success(self) -> Result<String, GitCommandError> {
        use git_command_error::*;

        snafu::ensure!(
            self.success,
            CommandFailedSnafu {
                command: self.command,
                exit_code: self.exit_code,
                stderr: self.stderr,
            }
        );
        Ok(self.stdout)
    }

    /// Returns stdout when the command succeeded and produced output.
    pub fn optional_stdout(self) -> Option<String> {
        self.success
            .then_some(self.stdout)
            .filter(|out| !out.is_empty())
    }
}
