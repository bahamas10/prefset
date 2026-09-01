/*!
 * `prefset` - Synchronize macOS preferences with one config file
 *
 * # Created
 * Author: Dave Eddy <ysap@daveeddy.com>
 * Date: August 31, 2026
 * License: MIT
 *
 * # Contributors
 * - Dave Eddy <ysap@daveeddy.com>
 */

#[cfg(not(target_os = "macos"))]
compile_error!("prefset only supports macOS");

use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::{env, io};

use anyhow::Result;

use crate::args::Command;

mod args;
mod commands;
mod config;
mod integrations;
mod util;

// CLI app config / state
pub struct State {
    pub color_enabled: bool,
    pub config_path: PathBuf,
}

fn main() -> Result<()> {
    // parse args and store some process state
    let cli = args::parse();
    let state = State {
        config_path: cli.config.to_owned(),
        color_enabled: color_enabled(),
    };

    match cli.command {
        None => commands::run(&state),
        Some(Command::Apply(cmd)) => commands::apply::run(&state, &cmd),
        Some(Command::Check(cmd)) => commands::check::run(&state, &cmd),
        Some(Command::Init(cmd)) => commands::init::run(&state, &cmd),
    }
}

fn color_enabled() -> bool {
    io::stdout().is_terminal()
        && env::var_os("NO_COLOR").is_none()
        && env::var_os("TERM").as_deref() != Some(OsStr::new("dumb"))
}
