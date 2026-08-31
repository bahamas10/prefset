/*!
 * `prefset check` subcommand
 */

use anyhow::{Result, bail};

use crate::State;
use crate::args::CheckCommand;
use crate::config::Config;
use crate::util::defaults;

/// `prefset check ...`
pub fn run(state: &State, _cmd: &CheckCommand) -> Result<()> {
    let config = Config::load(&state.config_path)?;

    let changes = defaults::diff(&config)?;
    let total = config.preferences.len();

    if !changes.is_empty() {
        eprintln!("{}/{} preferences not synced", changes.len(), total);
        bail!("differences detected");
    }

    println!("all preferences synced ({} total)", total);
    Ok(())
}
