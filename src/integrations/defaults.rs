/*!
 * Interface into macOS defaults using the `defaults(1)` CLI tool
 */

use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::process::Command;

use anyhow::{Context, Result, bail};
use plist::Dictionary;

use super::{DisplayValue, IntegrationChange};
use crate::util::shell;

pub const DEFAULTS_CMD: &str = "/usr/bin/defaults";

/// One configured key in a macOS defaults domain.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub domain: String,
    pub key: String,
    pub value: Value,
}

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

impl From<&Value> for DisplayValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Boolean(value) => Self::Boolean(*value),
            Value::Integer(value) => Self::Integer(*value),
            Value::Float(value) => Self::Float(*value),
            Value::String(value) => Self::String(value.clone()),
        }
    }
}

/// A single change to make to the system
#[derive(Clone, Debug)]
pub struct Change {
    setting: Setting,
    current: Option<Value>,
}

impl fmt::Display for Change {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let domain = &self.setting.domain;
        let key = &self.setting.key;
        let value = &self.setting.value;
        let current = match &self.current {
            Some(v) => format!("{v}"),
            None => "(unset)".into(),
        };

        write!(formatter, "{domain}.{key}: {current} -> {value}")
    }
}

impl IntegrationChange for Change {
    fn section(&self) -> String {
        section(&self.setting.domain)
    }

    fn key(&self) -> &str {
        &self.setting.key
    }

    fn current(&self) -> DisplayValue {
        self.current
            .as_ref()
            .map(DisplayValue::from)
            .unwrap_or(DisplayValue::Missing)
    }

    fn desired(&self) -> DisplayValue {
        DisplayValue::from(&self.setting.value)
    }

    fn is_applied(&self) -> bool {
        self.current.as_ref() == Some(&self.setting.value)
    }

    fn operation_hint(&self) -> Result<String> {
        write_command(&self.setting)
    }

    fn relaunches(&self) -> &'static [&'static str] {
        match self.setting.domain.as_str() {
            "com.apple.dock" => &["Dock"],
            "com.apple.finder" => &["Finder"],
            _ => &[],
        }
    }

    /// Apply a given change by forking the `defaults` command
    fn apply(&self) -> Result<()> {
        let setting = &self.setting;

        validate_domain(&setting.domain)?;

        let status = Command::new(DEFAULTS_CMD)
            .args([
                "write",
                &setting.domain,
                &setting.key,
                setting.value.defaults_type(),
                &setting.value.argument(),
            ])
            .status()
            .context("could not run defaults")?;

        if !status.success() {
            bail!(
                "defaults failed while writing {}.{}",
                setting.domain,
                setting.key
            );
        }

        Ok(())
    }
}

/// Parse the config value below the top-level `defaults` key.
pub fn parse(value: &toml::Value) -> Result<Vec<Setting>> {
    let domains = value
        .as_table()
        .context("the defaults namespace must contain domain tables")?;

    let mut settings = Vec::new();
    for (domain, contents) in domains {
        validate_domain(domain)?;
        let table = contents.as_table().with_context(|| {
            format!(
                "domain {domain:?} must be a table - quote dotted domain \
                 names, for example [defaults.\"com.apple.dock\"]"
            )
        })?;

        for (key, value) in table {
            let value = Value::from_toml(value).with_context(|| {
                format!("defaults.{domain}.{key} unsupported type")
            })?;
            settings.push(Setting {
                domain: domain.clone(),
                key: key.clone(),
                value,
            });
        }
    }

    Ok(settings)
}

/// Read current state and build an owned reconciliation plan.
pub fn plan(settings: &[Setting]) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let mut domains = HashMap::new();

    for setting in settings {
        // check if we have already looked at this domain - if not, look it
        // up and cache it
        if !domains.contains_key(&setting.domain) {
            let domain = export(&setting.domain)?;
            domains.insert(setting.domain.clone(), domain);
        }

        let current = domains[&setting.domain]
            .get(&setting.key)
            .and_then(value_from_plist);
        changes.push(Change { setting: setting.clone(), current });
    }

    Ok(changes)
}

/// Export an entire domain
fn export(domain: &str) -> Result<Dictionary> {
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

/// Write the command needed to enforce a preference safely for the shell
fn write_command(setting: &Setting) -> Result<String> {
    validate_domain(&setting.domain)?;
    let s = format!(
        "{} write {} {} {} {}",
        DEFAULTS_CMD,
        shell::quote(&setting.domain),
        shell::quote(&setting.key),
        setting.value.defaults_type(),
        shell::quote(&setting.value.argument())
    );
    Ok(s)
}

/// Convert the defaults domain to a printable section name
fn section(domain: &str) -> String {
    let bare = domain.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
    });

    if bare {
        format!("defaults.{domain}")
    } else {
        let quoted = toml::Value::String(domain.to_owned());
        format!("defaults.{quoted}")
    }
}

/// Convert a plist value to an internal value type
fn value_from_plist(value: &plist::Value) -> Option<Value> {
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
fn validate_domain(domain: &str) -> Result<()> {
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
