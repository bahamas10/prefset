/*!
 * `prefset init` subcommand
 */

use std::fs;

use anyhow::{Context, Result, bail};
use indoc::formatdoc;

use crate::State;
use crate::args::{self, InitCommand};
use crate::util::color::colorize;
use crate::util::path::friendly_path;

const MINIMAL_CONFIG: &str = include_str!("../../assets/minimal.toml");
const FULL_CONFIG: &str = include_str!("../../assets/full.toml");
const DAVE_CONFIG: &str = include_str!("../../assets/dave.toml");

/// `prefset init ...`
pub fn run(state: &State, cmd: &InitCommand) -> Result<()> {
    let color = state.color_enabled;
    let path = &state.config_path;

    // only allow non-custom configs to be init
    if &args::default_config_path() != path {
        bail!("init unsupported with `--config` option");
    }

    let friendly_path = friendly_path(path);
    let display_path = colorize(&friendly_path, "36", color);

    if path.exists() {
        // stop here if the config already exists
        let status = colorize("config already exists:", "1;33", color);

        eprint!(
            "{}",
            formatdoc! {"
        {status} {display_path}

        if you really want to re-init the config then remove it and re-run this
        command.

        "}
        );
        bail!("refusing to run");
    }

    // figure out the right config to write
    let contents = if cmd.full {
        FULL_CONFIG
    } else if cmd.dave {
        DAVE_CONFIG
    } else {
        MINIMAL_CONFIG
    };

    // save the config
    let parent =
        path.parent().context("configuration path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("could not create {}", parent.display()))?;
    fs::write(path, contents)
        .with_context(|| format!("could not create {}", path.display()))?;

    let apply_cmd = colorize("prefset apply --dry-run", 36, color);
    print!(
        "{}",
        formatdoc! {"
    created: {display_path}

    look over this file, modify it to your liking, then run:

        {apply_cmd}

    to get started.
    "}
    );

    Ok(())
}
