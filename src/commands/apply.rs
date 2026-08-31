/*!
 * `prefset apply` subcommand
 */

use std::collections::HashSet;
use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result, bail};
use indoc::formatdoc;

use crate::State;
use crate::args::ApplyCommand;
use crate::config::{Config, Preference};
use crate::util::color::colorize;
use crate::util::defaults::{self, Change};

const INDENT: &str = "    ";

/// `prefset apply ...`
pub fn run(state: &State, cmd: &ApplyCommand) -> Result<()> {
    let config = Config::load(&state.config_path)?;

    let color = state.color_enabled;

    let mut index = 0;
    let mut changed = 0;
    let mut first_domain = true;
    let mut to_relaunch = HashSet::new();
    let total = config.preferences.len();

    // loop every preference
    while index < total {
        if !first_domain {
            println!();
        }
        first_domain = false;

        // print the domain
        let domain = &config.preferences[index].domain;
        let header = domain_header(domain);
        println!("[{}]", colorize(&header, 36, color));

        // export the full domain
        let values = defaults::export(domain)?;

        // loop every preference while they are in the same domain - todo is
        // there a cleaner way to do this?
        while index < total && config.preferences[index].domain == *domain {
            let preference = &config.preferences[index];

            // get the "current" setting from the system (may be None)
            let current = values
                .get(&preference.key)
                .and_then(defaults::value_from_plist);

            // check if the preference is already in sync
            let already_synced = current.as_ref() == Some(&preference.value);
            let change = Change { preference: preference.clone(), current };

            /*
             * The program can be running in multiple different modes here:
             * - (default): apply the changes for settings not already in sync
             * - --force: apply the changes regardless of sync status
             * - --dry-run: ^ do what is above but don't actually execute it
             * - --verbose: ^ do above but print before running
             */
            let mut should_apply = false;
            if cmd.force {
                print_rewritten(preference, color);
                should_apply = true;
            } else if !already_synced {
                print_changed(&change, color);
                should_apply = true;
            } else {
                print_unchanged(preference, color);
            }

            // commit the change if needed
            if should_apply {
                changed += 1;

                // keep track of services that will need to be relauched
                match domain.as_str() {
                    "com.apple.dock" => {
                        to_relaunch.insert("Dock");
                    }
                    "com.apple.finder" => {
                        to_relaunch.insert("Finder");
                    }
                    _ => {}
                };

                if cmd.verbose {
                    let s = defaults::write_command(preference)?;
                    print_command(&s, color, cmd.dry_run);
                }

                // actually apply the setting
                if !cmd.dry_run {
                    // todo: change.apply() instead? make it a method of the
                    // struct?
                    defaults::apply(&change)?;
                }
            }

            index += 1;
        }
    }

    // preferences updated - print summary
    println!();
    print!("- updated {}/{} preferences", changed, total);
    if cmd.dry_run {
        print!(" (dry-run, nothing done)");
    }
    println!();

    // figure out if we need to relaunch any services
    if !to_relaunch.is_empty() {
        let to_relaunch: Vec<_> = to_relaunch.into_iter().collect();
        handle_relaunch(state, cmd, &to_relaunch)?;
        println!();
    }

    // done!
    let check = colorize("✓", 32, color);
    println!("{check} done");

    Ok(())
}

/**
 * Handle optionally relaunching processes affected by the preferences modified
 * based on the CLI flags given:
 *
 * - (default): io is a TTY: prompt the user yes or no
 * - (default): io is not a TTY: don't relaunch anything
 * - --relaunch: relaunch the services without prompting
 * - --no-relaunch: don't relaunch anything
 */
fn handle_relaunch(
    state: &State,
    cmd: &ApplyCommand,
    to_relaunch: &[&str],
) -> Result<()> {
    let color = state.color_enabled;
    let names = to_relaunch.join(", ");

    let header = colorize("----------------------------------------", 2, color);
    let count = colorize(&to_relaunch.len().to_string(), 35, color);

    println!();
    println!("{}", header);
    println!();
    println!("{} process(es) need to relaunch: {}", count, names);

    // stop here if the user says --no-relaunch
    if cmd.no_relaunch {
        println!(
            "skipping relaunch - `{}` given",
            colorize("--no-relaunch", 36, color)
        );
        return Ok(());
    }

    // stop here if not a TTY and the user didn't explicitly ask for relaunching
    let isatty = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !isatty && !cmd.relaunch {
        println!("skipping relaunch - input/output is not a TTY");
        return Ok(());
    }

    // prompt the user to confirm if `--relaunch` is not found
    if !cmd.relaunch {
        let relaunch_cmd = colorize(&relaunch_command(to_relaunch), 2, color);

        println!(
            "{}",
            formatdoc! {"

        > relaunching the services helps these changes take effect.
        > this will not log you out, but affected UI may briefly disappear.
        > will run: {relaunch_cmd}
        "}
        );

        print!("relaunch services now? [y/N]: ");
        io::stdout().flush().context("could not flush relaunch prompt")?;

        if cmd.dry_run {
            println!("n");
            println!(
                "(assuming no - `{}` is given)",
                colorize("--dry-run", 36, color)
            );
            return Ok(());
        }

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .context("could not read relaunch confirmation")?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!("not relaunching...");
            return Ok(());
        }
    }

    // if we are here then it's time to relaunch!
    if cmd.verbose {
        let s = relaunch_command(to_relaunch);
        print_command(&s, color, cmd.dry_run);
    }
    if !cmd.dry_run {
        let status = Command::new("/usr/bin/killall")
            .args(to_relaunch)
            .status()
            .with_context(|| format!("could not relaunch {}", names))?;
        if !status.success() {
            bail!("could not relaunch {}", names);
        }

        println!("relaunched {}", names);
    }

    Ok(())
}

/// Return the stringified version of the "killall" command
fn relaunch_command(processes: &[&str]) -> String {
    let mut args = vec!["/usr/bin/killall".to_string()];
    for proc in processes {
        args.push(crate::util::shell::quote(proc));
    }

    args.join(" ")
}

/// Format the domain header (safely quote it)
fn domain_header(domain: &str) -> String {
    // figure out if we need to quote the header
    let bare = domain.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
    });

    if bare {
        format!("defaults.{}", domain)
    } else {
        let quoted = toml::Value::String(domain.to_owned());
        format!("defaults.{}", quoted)
    }
}

/// Print a command to the screen
fn print_command(command: &str, color: bool, dry_run: bool) {
    let cmd = colorize(command, 2, color);

    let state = match dry_run {
        true => colorize("[SKIPPED]", 2, color),
        false => colorize("[RUN]", 2, color),
    };

    println!("{INDENT}{INDENT}{} {}\n", state, cmd);
}

/// Print a message when a preference is *not* updated
fn print_unchanged(preference: &Preference, color: bool) {
    let check = colorize("✓", 32, color);
    let key = &preference.key;
    let value = &preference.value;

    let msg = format!("{} = {}", key, value);
    let msg = colorize(&msg, 2, color);

    println!("{INDENT}{check} {}", msg);
}

/// Print a message when a preference is updated
fn print_changed(change: &Change, color: bool) {
    let arrow = colorize("→", 33, color);
    let key = &change.preference.key;

    let current = change
        .current
        .as_ref()
        .map(|value| styled_value(value, color))
        .unwrap_or_else(|| colorize("<unset>", 2, color));

    let desired = styled_value(&change.preference.value, color);

    println!("{INDENT}{arrow} {}: {} -> {}", key, current, desired);
}

/// Print a message when a preference is forced rewritten
fn print_rewritten(preference: &Preference, color: bool) {
    let rewrite = colorize("o", 35, color);
    let key = &preference.key;
    let value = styled_value(&preference.value, color);

    println!("{INDENT}{rewrite} {} = {}", key, value);
}

/// Colorize values consistently
fn styled_value(value: &defaults::Value, color: bool) -> String {
    let ansi_color = match value {
        defaults::Value::Boolean(_) => 35,
        defaults::Value::Integer(_) | defaults::Value::Float(_) => 36,
        defaults::Value::String(_) => 32,
    };
    colorize(&value.to_string(), ansi_color, color)
}
