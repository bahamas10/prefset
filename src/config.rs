/*!
 * Loading and dispatching the top-level keys in `config.toml`.
 *
 * Each integration owns the shape and validation of the value below its key.
 */

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::integrations::{appearance, defaults, filesystem, wallpaper};

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub defaults: Vec<defaults::Setting>,
    pub appearance: Option<appearance::Setting>,
    pub filesystem: Vec<filesystem::Setting>,
    pub wallpaper: Option<wallpaper::Setting>,
}

impl Config {
    /// Load a configuration file by path.
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;

        Self::parse(&source).with_context(|| {
            format!("invalid configuration in {}", path.display())
        })
    }

    /// Parse a configuration document and dispatch each supported top-level key.
    pub fn parse(source: &str) -> Result<Self> {
        let document: toml::Table =
            toml::from_str(source).context("invalid TOML")?;

        let mut defaults_settings = Vec::new();
        let mut appearance_setting = None;
        let mut filesystem_settings = Vec::new();
        let mut wallpaper_setting = None;

        for (key, value) in document {
            match key.as_str() {
                "appearance" => {
                    appearance_setting = Some(appearance::parse(&value)?);
                }
                "defaults" => {
                    defaults_settings = defaults::parse(&value)?;
                }
                "filesystem" => {
                    filesystem_settings = filesystem::parse(&value)?;
                }
                "wallpaper" => {
                    wallpaper_setting = Some(wallpaper::parse(&value)?);
                }
                _ => bail!("unsupported integration: {:?}", key),
            }
        }

        Ok(Self {
            defaults: defaults_settings,
            appearance: appearance_setting,
            filesystem: filesystem_settings,
            wallpaper: wallpaper_setting,
        })
    }
}
