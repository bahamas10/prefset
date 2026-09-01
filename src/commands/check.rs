/*!
 * `prefset check` subcommand
 */

use anyhow::{Result, bail};

use crate::State;
use crate::args::CheckCommand;
use crate::config::Config;
use crate::integrations::{self, IntegrationChange};

/// `prefset check ...`
pub fn run(state: &State, _cmd: &CheckCommand) -> Result<()> {
    let config = Config::load(&state.config_path)?;
    let plan = integrations::plan(&config)?;

    let differences = plan.iter().filter(|change| !change.is_applied()).count();
    let total = plan.len();

    if differences != 0 {
        eprintln!("{}/{} preferences not synced", differences, total);
        bail!("differences detected");
    }

    println!("all preferences synced ({} total)", total);
    Ok(())
}
