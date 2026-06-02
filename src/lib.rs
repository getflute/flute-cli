#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod cli;
pub mod config;

pub fn run() -> anyhow::Result<()> {
    Ok(())
}
