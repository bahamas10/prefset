/*!
 * Argument parsing for `prefset`
 */

use std::env;
use std::io;
use std::path::PathBuf;

use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser, Subcommand};
use indoc::indoc;

// add some colors to the clap help message
const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default())
    .placeholder(AnsiColor::Cyan.on_default())
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Yellow.on_default());

// examples at the bottom of a help message are just a cheat code imo i love
// them
const AFTER_HELP: &str = indoc! {"
\x1b[1;32mExamples:\x1b[0m
  \x1b[36mprefset check           \x1b[0m    Check if preferences are set
  \x1b[36mprefset apply --dry-run \x1b[0m    See what changes would be made
  \x1b[36mprefset apply           \x1b[0m    Actually set your preferences
"};

#[derive(Debug, Parser)]
#[command(
    version,
    about = env!("CARGO_PKG_DESCRIPTION"),
    styles = STYLES,
    after_help = AFTER_HELP
)]
pub struct Cli {
    /// Config file
    #[arg(
        long,
        value_name = "PATH",
        default_value_os_t = default_config_path()
    )]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a starter config file
    Init(InitCommand),

    /// Exit successfully if all configured preferences are set
    Check(CheckCommand),

    /// Apply configured preferences
    Apply(ApplyCommand),

    /// Print an equivalent standalone shell script
    Export(ExportCommand),
}

#[derive(Debug, Default, Parser)]
pub struct InitCommand {
    /// Create a minimal starter configuration (default)
    #[arg(long, group = "template")]
    pub minimal: bool,

    /// Create a comprehensive example configuration
    #[arg(long, group = "template")]
    pub full: bool,

    /// Create Dave's opinionated configuration
    #[arg(long, group = "template")]
    pub dave: bool,
}

#[derive(Debug, Default, Parser)]
pub struct CheckCommand {}

#[derive(Debug, Default, Parser)]
pub struct ApplyCommand {
    /// Show changes without writing them
    #[arg(long)]
    pub dry_run: bool,

    /// Increase verbosity (shows commands being executed)
    #[arg(long)]
    pub verbose: bool,

    /// Always write preferences regardless if they are already set
    #[arg(long)]
    pub force: bool,

    /// Restart affected macOS processes without prompting
    #[arg(long, group = "relaunch-policy")]
    pub relaunch: bool,

    /// Do not relaunch affected macOS processes or prompt
    #[arg(long, group = "relaunch-policy")]
    pub no_relaunch: bool,
}

#[derive(Debug, Default, Parser)]
pub struct ExportCommand {}

pub fn parse() -> Cli {
    Cli::parse()
}

pub fn print_help() -> io::Result<()> {
    Cli::command().print_help()
}

pub fn default_config_path() -> PathBuf {
    let home = env::home_dir().expect("failed to find home dir");
    home.join(".config/prefset/config.toml")
}
