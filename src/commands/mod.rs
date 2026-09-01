/*!
 * `prefset` bare subcommand
 */

use anyhow::{Result, bail};
use indoc::formatdoc;

use crate::util::color::colorize;
use crate::{State, args, util};

pub mod apply;
pub mod check;
pub mod init;

/// `prefset` called with no arguments
pub fn run(state: &State) -> Result<()> {
    // only allow non-custom configs to be init
    if args::default_config_path() != state.config_path {
        bail!("bare command unsupported with `--config` option");
    }

    if state.config_path.exists() {
        print_has_config(state)
    } else {
        print_no_config(state)
    }
}

fn print_has_config(state: &State) -> Result<()> {
    let color = state.color_enabled;
    let path = &state.config_path;

    let friendly_path = util::path::friendly_path(path);
    let display_path = colorize(&friendly_path, "36", color);
    let status = colorize("config file found:", "1;33", color);
    println!("{status} {display_path}\n");
    args::print_help()?;

    Ok(())
}

fn print_no_config(state: &State) -> Result<()> {
    let color = state.color_enabled;
    let path = &state.config_path;
    let friendly_path = util::path::friendly_path(path);

    let heading = "to get started, create and test a minimal config with:";

    let init = "prefset init";
    let edit = format!("$EDITOR {}", friendly_path);
    let apply = "prefset apply --dry-run";

    let init = colorize(init, "36", color);
    let edit = colorize(&edit, "36", color);
    let apply = colorize(apply, "36", color);

    let root_help = "prefset -h";
    let apply_help = "prefset apply -h";

    print!(
        "{}",
        formatdoc! {"
    {heading}

        {init}
        {edit}
        {apply}

    run `{root_help}` or `{apply_help}` to see more options
    "}
    );

    Ok(())
}
