/*!
 * Manage the macOS system appearance through System Events.
 */

use std::fmt;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::{DisplayValue, IntegrationChange};
use crate::util::shell;

const OSASCRIPT_CMD: &str = "/usr/bin/osascript";
const READ_SCRIPT: &str =
    r#"Application("System Events").appearancePreferences.darkMode()"#;

/// The configured system appearance
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    fn dark_mode(self) -> bool {
        matches!(self, Self::Dark)
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Light => "light",
            Self::Dark => "dark",
        };
        write!(f, "{}", s)
    }
}

/// The config below the top-level `appearance` key.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub mode: Mode,
}

/// A planned system appearance change.
#[derive(Clone, Debug)]
pub struct Change {
    desired: Mode,
    current: Mode,
}

impl IntegrationChange for Change {
    fn section(&self) -> String {
        "appearance".to_owned()
    }

    fn key(&self) -> &str {
        "mode"
    }

    fn current(&self) -> DisplayValue {
        DisplayValue::String(self.current.to_string())
    }

    fn desired(&self) -> DisplayValue {
        DisplayValue::String(self.desired.to_string())
    }

    fn is_applied(&self) -> bool {
        self.current == self.desired
    }

    fn apply(&self) -> Result<()> {
        let script = write_script(self.desired);
        let _ = run_javascript(&script)
            .context("could not set system appearance")?;

        Ok(())
    }

    fn operation_hint(&self) -> Result<String> {
        Ok(write_command(&write_script(self.desired)))
    }

    fn relaunches(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Parse the value below the top-level `appearance` key.
pub fn parse(value: &toml::Value) -> Result<Setting> {
    let table = value.as_table().context("appearance must be a table")?;

    for key in table.keys() {
        if key != "mode" {
            bail!("appearance.{key} is unsupported");
        }
    }

    let mode = table
        .get("mode")
        .context("appearance.mode is required")?
        .as_str()
        .context("appearance.mode must be a string")?;

    let mode = match mode {
        "light" => Mode::Light,
        "dark" => Mode::Dark,
        _ => bail!("appearance.mode must be \"light\" or \"dark\""),
    };

    Ok(Setting { mode })
}

/// Read current state and build an owned reconciliation plan.
pub fn plan(setting: &Setting) -> Result<Change> {
    let output = run_javascript(READ_SCRIPT)
        .context("could not read system appearance")?;

    // parse the output - we are asking specifically "are you dark mode?"
    let current = match output.trim() {
        "true" => Mode::Dark,
        "false" => Mode::Light,
        v => {
            bail!("osascript returned an invalid dark mode value: {v:?}")
        }
    };

    Ok(Change { desired: setting.mode, current })
}

/// Run the javascript string and return the output as a UTF-8 string
fn run_javascript(script: &str) -> Result<String> {
    let output = Command::new(OSASCRIPT_CMD)
        .args(["-l", "JavaScript", "-e", script])
        .output()
        .context("could not run osascript")?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        bail!(
            "osascript failed while setting system appearance: {}",
            message.trim()
        );
    }

    let output = String::from_utf8_lossy(&output.stdout);
    Ok(output.to_string())
}

/// Create the javascript script to run
fn write_script(mode: Mode) -> String {
    format!(
        r#"Application("System Events").appearancePreferences.darkMode = {}"#,
        mode.dark_mode()
    )
}

/// Write the command needed to enforce this - used only for display
fn write_command(script: &str) -> String {
    format!("{OSASCRIPT_CMD} -l JavaScript -e {}", shell::quote(script))
}
