/*!
 * All of the logic regarding the prefset `config.toml` file
 */

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::util::defaults;

/**
 * The "one config file" is currently just a large vec of "preferences" where
 * each line is its own preference.
 */
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub preferences: Vec<Preference>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preference {
    pub domain: String,
    pub key: String,
    pub value: defaults::Value,
}

impl Config {
    /// Load a configuration file by path
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;

        Self::parse(&source).with_context(|| {
            format!("invalid configuration in {}", path.display())
        })
    }

    /// Parse a config as a string
    pub fn parse(source: &str) -> Result<Self> {
        let document: toml::Table =
            toml::from_str(source).context("invalid TOML")?;

        let mut preferences = Vec::new();

        // get the "defaults" values
        let contents =
            document.get("defaults").context("'defaults' domain not found")?;

        // loop domains
        let domains = contents
            .as_table()
            .context("the defaults namespace must contain domain tables")?;
        for (domain, contents) in domains {
            defaults::validate_domain(domain)?;

            let table = contents.as_table().with_context(|| {
                format!(
                    "domain {domain:?} must be a table - quote dotted domain \
                         names, for example [defaults.\"com.apple.dock\"]"
                )
            })?;

            // loop preferences
            for (key, value) in table {
                let value =
                    defaults::Value::from_toml(value).with_context(|| {
                        format!("defaults.{domain}.{key} unsupported type")
                    })?;

                preferences.push(Preference {
                    domain: domain.clone(),
                    key: key.clone(),
                    value,
                });
            }
        }

        Ok(Self { preferences })
    }
}
