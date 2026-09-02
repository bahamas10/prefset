/*!
 * Integrations turn configuration into owned, executable reconciliation plans.
 */

use std::fmt;

use anyhow::Result;
use enum_dispatch::enum_dispatch;

use crate::config::Config;

pub mod appearance;
pub mod defaults;
pub mod filesystem;
pub mod wallpaper;

/// An integration-neutral value prepared for command-line presentation.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Missing,
    Description(String),
}

impl fmt::Display for DisplayValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boolean(value) => write!(formatter, "{value}"),
            Self::Integer(value) => write!(formatter, "{value}"),
            Self::Float(value) => write!(formatter, "{value}"),
            Self::String(value) => write!(formatter, "{value:?}"),
            Self::Missing => write!(formatter, "<unset>"),
            Self::Description(value) => formatter.write_str(value),
        }
    }
}

/// The shared behavior exposed by every integration-specific change.
#[enum_dispatch]
pub trait IntegrationChange {
    fn section(&self) -> String;
    fn key(&self) -> &str;
    fn current(&self) -> DisplayValue;
    fn desired(&self) -> DisplayValue;
    fn is_applied(&self) -> bool;
    fn apply(&self) -> Result<()>;
    fn operation_hint(&self) -> Result<String>;
    fn relaunches(&self) -> &'static [&'static str];
}

/// One planned unit of work, with current state captured before any mutation.
#[enum_dispatch(IntegrationChange)]
#[derive(Clone, Debug)]
pub enum PlannedChange {
    Appearance(appearance::Change),
    Defaults(defaults::Change),
    Filesystem(filesystem::Change),
    Wallpaper(wallpaper::Change),
}

/// Build the complete plan before applying any changes.
pub fn plan(config: &Config) -> Result<Vec<PlannedChange>> {
    let mut plan = vec![];

    if let Some(setting) = &config.appearance {
        plan.push(appearance::plan(setting)?.into());
    }

    plan.extend(
        defaults::plan(&config.defaults)?.into_iter().map(PlannedChange::from),
    );

    plan.extend(
        filesystem::plan(&config.filesystem)?
            .into_iter()
            .map(PlannedChange::from),
    );

    if let Some(setting) = &config.wallpaper {
        plan.push(wallpaper::plan(setting)?.into());
    }

    Ok(plan)
}
