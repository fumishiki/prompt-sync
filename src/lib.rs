mod app;
mod cli;
pub(crate) mod config;
pub(crate) mod engine;
pub(crate) mod logging;
pub(crate) mod model;
pub(crate) mod pathing;
pub(crate) mod safe_fs;
pub(crate) mod vcs;

pub use crate::cli::{Cli, Command, Profile};

pub fn run(cli: Cli) -> anyhow::Result<i32> {
    app::run(cli)
}
