/*!
 * Interface into macOS defaults using the `defaults(1)` CLI tool
 */

use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::process::Command;

use anyhow::{Context, Result, bail};
use plist::Dictionary;

use crate::config::{Config, Preference};
use crate::util::shell;

pub const DEFAULTS_CMD: &str = "/usr/bin/defaults";

/**
 * This is a bit confusing, but this program makes use of 3 `Value` enums:
 *
 * - [`plist::Value`] - The value stored in plist
 * - [`toml::Value`] - The value stored in TOML
 * - [`defaults::Value`] - Our own internal version of a value that can be
 *   easily given to macOS `defaults(1)`
 */
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl Value {
    /// Convert a TOML value to this type
    pub fn from_toml(value: &toml::Value) -> Result<Self> {
        match value {
            toml::Value::Boolean(value) => Ok(Self::Boolean(*value)),
            toml::Value::Integer(value) => Ok(Self::Integer(*value)),
            toml::Value::Float(value) => {
                if value.is_finite() {
                    Ok(Self::Float(*value))
                } else {
                    bail!("non-finite floats are not supported: {:?}", value)
                }
            }
            toml::Value::String(value) => Ok(Self::String(value.clone())),
            _ => bail!("unsupported value type"),
        }
    }

    /// Get the "type" arguments to the `defaults` command
    pub fn defaults_type(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "-bool",
            Self::Integer(_) => "-int",
            Self::Float(_) => "-float",
            Self::String(_) => "-string",
        }
    }

    /// Get the value as an owned value suitable to passing to the `defaults`
    /// command as an argument
    pub fn argument(&self) -> String {
        match self {
            Self::Boolean(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::String(value) => value.clone(),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => write!(formatter, "{}", value),
            Self::Integer(value) => write!(formatter, "{}", value),
            Self::Float(value) => write!(formatter, "{}", value),
            Self::String(value) => write!(formatter, "{:?}", value),
        }
    }
}

/// A single change to make to the system
#[derive(Clone, Debug)]
pub struct Change {
    pub preference: Preference,
    pub current: Option<Value>,
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let domain = &self.preference.domain;
        let key = &self.preference.key;
        let value = &self.preference.value;
        let current = match &self.current {
            Some(v) => format!("{v}"),
            None => "(unset)".into(),
        };

        write!(formatter, "{domain}.{key}: {current} -> {value}")
    }
}

/// Calculate changes needed to reconcile a system
pub fn diff(config: &Config) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let mut domains = HashMap::new();

    // loop over every preference found in the config
    for preference in &config.preferences {
        // check if we have already looked at this domain - if not, look it
        // up and cache it
        if !domains.contains_key(&preference.domain) {
            let domain = export(&preference.domain)?;
            domains.insert(preference.domain.clone(), domain);
        }

        let current = domains[&preference.domain]
            .get(&preference.key)
            .and_then(value_from_plist);

        // only store the change if what the config has differs from what
        // the system has
        if current.as_ref() != Some(&preference.value) {
            changes.push(Change { preference: preference.clone(), current });
        }
    }
    Ok(changes)
}

/// Export an entire domain
pub fn export(domain: &str) -> Result<Dictionary> {
    validate_domain(domain)?;

    let output = Command::new(DEFAULTS_CMD)
        .args(["export", domain, "-"])
        .output()
        .context("could not run defaults")?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        bail!(
            "defaults could not export domain {:?}: {}",
            domain,
            message.trim()
        );
    }

    let value = plist::Value::from_reader_xml(Cursor::new(output.stdout))
        .with_context(|| {
            format!(
                "defaults returned an invalid plist for domain {:?}",
                domain
            )
        })?;

    value.into_dictionary().with_context(|| {
        format!(
            "defaults returned a non-dictionary plist for domain {:?}",
            domain
        )
    })
}

/// Apply a given change by forking the `defaults` command
pub fn apply(change: &Change) -> Result<()> {
    let preference = &change.preference;

    validate_domain(&preference.domain)?;

    let status = Command::new(DEFAULTS_CMD)
        .args([
            "write",
            &preference.domain,
            &preference.key,
            preference.value.defaults_type(),
            &preference.value.argument(),
        ])
        .status()
        .context("could not run defaults")?;

    if !status.success() {
        bail!(
            "defaults failed while writing {}.{}",
            preference.domain,
            preference.key
        );
    }

    Ok(())
}

/// Write the command needed to enforce a preference safely for the shell
pub fn write_command(preference: &Preference) -> Result<String> {
    validate_domain(&preference.domain)?;
    let s = format!(
        "{} write {} {} {} {}",
        DEFAULTS_CMD,
        shell::quote(&preference.domain),
        shell::quote(&preference.key),
        preference.value.defaults_type(),
        shell::quote(&preference.value.argument())
    );
    Ok(s)
}

/// Convert a plist value to an internal value type
pub fn value_from_plist(value: &plist::Value) -> Option<Value> {
    match value {
        plist::Value::Boolean(value) => Some(Value::Boolean(*value)),
        plist::Value::Integer(value) => value.as_signed().map(Value::Integer),
        plist::Value::Real(value) => Some(Value::Float(*value)),
        plist::Value::String(value) => Some(Value::String(value.clone())),
        _ => None,
    }
}

/**
 * Validate that a defaults domain is valid
 *
 * This is super unfortunate, but because we ultimately rely on the `defaults`
 * command we need to be absolutely sure that what a user gives us does *not*
 * look like a file.  This is because you can read both a domain or a file with
 * `defaults`.  For Example:
 *
 *   - `defaults read ./foo.plist`      <- fails
 *   - `defaults read "$PWD/foo.plist"` <- works
 *
 * so to avoid any potential file manipulation we do some basic validation.
 */
pub fn validate_domain(domain: &str) -> Result<()> {
    if domain.is_empty() {
        bail!("preference domain may not be empty");
    }

    if domain.starts_with('-') {
        bail!("preference domain may not start with '-': {:?}", domain);
    }

    if domain.contains('/') {
        bail!("preference domain may not contain '/': {:?}", domain);
    }

    if domain.chars().any(char::is_control) {
        bail!(
            "preference domain may not contain control characters: {:?}",
            domain
        );
    }

    Ok(())
}
