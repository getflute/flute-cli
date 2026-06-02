//! Clap argument tree for the `flute` binary.

use clap::{Parser, Subcommand};

pub mod auth;
pub mod money;
pub mod output;
pub mod transactions;
pub mod util;

pub use output::OutputFormat;

#[derive(Parser, Debug)]
#[command(name = "flute", version, about = "CLI for the Flute payments platform")]
pub struct Cli {
    /// Active profile (environment). `sandbox` (default) or `production`/`prod`.
    #[arg(long, env = "FLUTE_PROFILE", default_value = "sandbox", global = true)]
    pub profile: String,

    /// Output format: table (default), json, or quiet (id only).
    /// When omitted, falls back to the `output` key in ~/.flute/config.toml,
    /// then to `table`.
    #[arg(long, global = true, value_enum)]
    pub output: Option<OutputFormat>,

    /// ISV merchant context for commands whose endpoints accept it.
    #[arg(long, global = true)]
    pub merchant_id: Option<String>,

    /// Print full HTTP request/response (sensitive fields redacted) to stderr.
    #[arg(long, global = true)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Authentication and profile management.
    #[command(subcommand)]
    Auth(AuthCommand),
    /// API health check.
    Ping,
    /// Print CLI version and active profile.
    Version,
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// Prompt for client_id + client_secret and store them in the OS keychain.
    Login,
    /// Show active profile, environment, and token status.
    Status,
    /// Set the default profile in ~/.flute/config.toml.
    Switch { profile: String },
    /// Clear stored credentials for the active profile.
    Logout,
    /// Print the current bearer token (debugging aid).
    Token,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
