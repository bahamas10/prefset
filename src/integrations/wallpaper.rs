/*!
 * Manage desktop wallpaper through macOS AppKit.
 */

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use objc2::MainThreadMarker;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSScreen, NSWorkspace, NSWorkspaceDesktopImageOptionKey};
use objc2_foundation::{NSDictionary, NSString, NSURL};

use super::{DisplayValue, IntegrationChange};
use crate::util::path::{expand_home, friendly_path};

/// The configuration below the top-level `wallpaper` key.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub path: PathBuf,
}

/// An owned snapshot of the current and desired wallpaper state.
#[derive(Clone, Debug)]
pub struct Change {
    desired: PathBuf,
    current: Vec<Option<PathBuf>>,
}

impl IntegrationChange for Change {
    fn section(&self) -> String {
        "wallpaper".to_owned()
    }

    fn key(&self) -> &str {
        "path"
    }

    fn current(&self) -> DisplayValue {
        let Some(first) = self.current.first() else {
            return DisplayValue::Missing;
        };

        if self.current.iter().any(|current| current != first) {
            return DisplayValue::Description("<varies by screen>".to_owned());
        }

        first
            .as_ref()
            .map(|path| DisplayValue::String(friendly_path(path)))
            .unwrap_or(DisplayValue::Missing)
    }

    fn desired(&self) -> DisplayValue {
        DisplayValue::String(friendly_path(&self.desired))
    }

    fn is_applied(&self) -> bool {
        !self.current.is_empty()
            && self
                .current
                .iter()
                .all(|current| current.as_deref() == Some(&self.desired))
    }

    fn operation_hint(&self) -> Result<String> {
        Ok(format!("NSWorkspace setDesktopImageURL {:?}", self.desired))
    }

    fn relaunches(&self) -> &'static [&'static str] {
        &[]
    }

    fn apply(&self) -> Result<()> {
        let path = self
            .desired
            .to_str()
            .context("wallpaper path is not valid Unicode")?;
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        let options =
            NSDictionary::<NSWorkspaceDesktopImageOptionKey, AnyObject>::new();
        let workspace = NSWorkspace::sharedWorkspace();

        for screen in screens()?.to_vec() {
            // SAFETY: The dictionary is empty, so it satisfies AppKit's requirement
            // that every key and value be a desktop-image option of the correct type.
            unsafe {
                workspace
                    .setDesktopImageURL_forScreen_options_error(
                        &url, &screen, &options,
                    )
                    .map_err(|error| {
                        anyhow::anyhow!(
                            error.localizedDescription().to_string()
                        )
                    })?;
            }
        }

        Ok(())
    }
}

/// Read current state and build an owned reconciliation plan.
pub fn plan(setting: &Setting) -> Result<Change> {
    let desired = resolve(&setting.path)?;
    let workspace = NSWorkspace::sharedWorkspace();

    let current = screens()?
        .to_vec()
        .iter()
        .map(|screen| {
            workspace
                .desktopImageURLForScreen(screen)
                .as_deref()
                .and_then(url_path)
                .and_then(|path| path.canonicalize().ok())
        })
        .collect();

    Ok(Change { desired, current })
}

/// Parse the value below the top-level `wallpaper` key.
pub fn parse(value: &toml::Value) -> Result<Setting> {
    let table = value.as_table().context("wallpaper must be a table")?;

    for key in table.keys() {
        if key != "path" {
            bail!("wallpaper.{key} is unsupported");
        }
    }

    let path = table
        .get("path")
        .context("wallpaper.path is required")?
        .as_str()
        .context("wallpaper.path must be a string")?;

    if path.is_empty() {
        bail!("wallpaper.path may not be empty");
    }

    Ok(Setting { path: PathBuf::from(path) })
}

/// Expand and validate a configured wallpaper path.
fn resolve(configured: &Path) -> Result<PathBuf> {
    let path = expand_home(configured)?;

    if !path.is_absolute() {
        bail!(
            "wallpaper.path must be absolute or begin with '~': {}",
            configured.display()
        );
    }

    let path = path.canonicalize().with_context(|| {
        format!("could not resolve wallpaper image {}", path.display())
    })?;

    if !path.is_file() {
        bail!("wallpaper path is not a file: {}", path.display());
    }

    Ok(path)
}

/// List all screens currently connected
fn screens() -> Result<objc2::rc::Retained<objc2_foundation::NSArray<NSScreen>>>
{
    let marker = MainThreadMarker::new()
        .context("wallpaper operations must run on the main thread")?;

    let screens = NSScreen::screens(marker);
    if screens.is_empty() {
        bail!("macOS did not report any connected screens");
    }

    Ok(screens)
}

/// NSURL -> PathBuf helper
fn url_path(url: &NSURL) -> Option<PathBuf> {
    url.path().map(|path| PathBuf::from(path.to_string()))
}
