/*!
 * `prefset apply` subcommand
 */

use std::collections::BTreeSet;
use std::io;
use std::io::IsTerminal;
use std::io::Write;
use std::process::Command;

use anyhow::{Context, Result, bail};
use indoc::formatdoc;

use crate::State;
use crate::args::ApplyCommand;
use crate::config::Config;
use crate::integrations::{
    self, DisplayValue, IntegrationChange, PlannedChange,
};
use crate::util::color::colorize;

const INDENT: &str = "    ";

/// `prefset apply ...`
pub fn run(state: &State, cmd: &ApplyCommand) -> Result<()> {
    let color = state.color_enabled;

    // parse the config and come up with a plan
    let config = Config::load(&state.config_path)?;
    let plan = integrations::plan(&config)?;

    let mut changed = 0;
    let mut to_relaunch = BTreeSet::new();
    let mut previous_section = None;
    let total = plan.len();

    for change in &plan {
        let section = change.section();

        // print the section "header" if it's a new section
        if previous_section.as_ref() != Some(&section) {
            if previous_section.is_some() {
                println!();
            }
            println!("[{}]", colorize(&section, 36, color));
            previous_section = Some(section);
        }

        let is_applied = change.is_applied();
        let should_apply = cmd.force || !is_applied;

        // print a message
        if cmd.force {
            print_rewritten(change, color);
        } else if is_applied {
            print_unchanged(change, color);
        } else {
            print_changed(change, color);
        }

        if !should_apply {
            continue;
        }

        // keep track of what to relaunch (if any)
        for process in change.relaunches() {
            to_relaunch.insert(*process);
        }

        if cmd.verbose {
            print_operation(&change.operation_hint()?, color, cmd.dry_run);
        }

        if !cmd.dry_run {
            // make the change
            change.apply()?;
        }

        changed += 1;
    }

    println!();
    print!("- updated {changed}/{total} preferences");
    if cmd.dry_run {
        print!(" (dry-run, nothing done)");
    }
    println!();

    if !to_relaunch.is_empty() {
        let to_relaunch: Vec<_> = to_relaunch.into_iter().collect();
        handle_relaunch(state, cmd, &to_relaunch)?;
        println!();
    }

    let check = colorize("✓", 32, color);
    println!("{check} done");

    Ok(())
}

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
    println!("{header}");
    println!();
    println!("{count} process(es) need to relaunch: {names}");

    if cmd.no_relaunch {
        println!(
            "skipping relaunch - `{}` given",
            colorize("--no-relaunch", 36, color)
        );
        return Ok(());
    }

    let isatty = io::stdin().is_terminal() && io::stdout().is_terminal();
    if !isatty && !cmd.relaunch {
        println!("skipping relaunch - input/output is not a TTY");
        return Ok(());
    }

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

    if cmd.verbose {
        print_operation(&relaunch_command(to_relaunch), color, cmd.dry_run);
    }

    if !cmd.dry_run {
        let status = Command::new("/usr/bin/killall")
            .args(to_relaunch)
            .status()
            .with_context(|| format!("could not relaunch {names}"))?;
        if !status.success() {
            bail!("could not relaunch {names}");
        }

        println!("relaunched {names}");
    }

    println!();
    println!("> note that some changes may not take affect until the user");
    println!("> logs out and logs back in.");

    Ok(())
}

fn relaunch_command(processes: &[&str]) -> String {
    let mut args = vec!["/usr/bin/killall".to_owned()];
    for process in processes {
        args.push(crate::util::shell::quote(process));
    }
    args.join(" ")
}

fn print_operation(operation: &str, color: bool, dry_run: bool) {
    let operation = colorize(operation, 2, color);
    let state = if dry_run {
        colorize("[SKIPPED]", 2, color)
    } else {
        colorize("[RUN]", 2, color)
    };

    println!("{INDENT}{INDENT}{state} {operation}\n");
}

fn print_unchanged(change: &PlannedChange, color: bool) {
    let check = colorize("✓", 32, color);
    let property = format!("{} = {}", change.key(), change.desired());
    let property = colorize(&property, 2, color);
    println!("{INDENT}{check} {property}");
}

fn print_changed(change: &PlannedChange, color: bool) {
    let arrow = colorize("→", 33, color);
    let current = styled_value(&change.current(), color);
    let desired = styled_value(&change.desired(), color);
    println!("{INDENT}{arrow} {}: {current} -> {desired}", change.key());
}

fn print_rewritten(change: &PlannedChange, color: bool) {
    let rewrite = colorize("o", 35, color);
    let value = styled_value(&change.desired(), color);
    println!("{INDENT}{rewrite} {} = {value}", change.key());
}

fn styled_value(value: &DisplayValue, color: bool) -> String {
    let ansi_color = match value {
        DisplayValue::Boolean(_) => 35,
        DisplayValue::Integer(_) | DisplayValue::Float(_) => 36,
        DisplayValue::String(_) => 32,
        DisplayValue::Missing | DisplayValue::Description(_) => 2,
    };
    colorize(&value.to_string(), ansi_color, color)
}
