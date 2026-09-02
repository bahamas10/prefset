/*!
 * Manage macOS file flags.
 *
 * This currently just handles hiding/unhiding
 */

use std::ffi::{CString, c_char, c_int};
use std::fs;
use std::os::macos::fs::MetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::{DisplayValue, IntegrationChange};
use crate::util::path::{expand_home, friendly_path};
use crate::util::shell;

unsafe extern "C" {
    fn lchflags(path: *const c_char, flags: u32) -> c_int;
}

// From macOS <sys/stat.h>. This is part of the stable file-flags ABI used by
// `stat(2)` and `lchflags(3)`.
const UF_HIDDEN: u32 = 0x0000_8000;

/// One configured filesystem path.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub path: PathBuf,
    pub hidden: bool,
}

/// A planned change to one filesystem flag.
#[derive(Clone, Debug)]
pub struct Change {
    path: PathBuf,
    desired: bool,
    current: bool,
}

impl IntegrationChange for Change {
    fn section(&self) -> String {
        let path = toml::Value::String(friendly_path(&self.path));
        format!("filesystem.{path}")
    }

    fn key(&self) -> &str {
        "hidden"
    }

    fn current(&self) -> DisplayValue {
        DisplayValue::Boolean(self.current)
    }

    fn desired(&self) -> DisplayValue {
        DisplayValue::Boolean(self.desired)
    }

    fn is_applied(&self) -> bool {
        self.current == self.desired
    }

    fn apply(&self) -> Result<()> {
        set_hidden(&self.path, self.desired)
    }

    fn operation_hint(&self) -> Result<String> {
        let flag = if self.desired { "hidden" } else { "nohidden" };
        Ok(format!(
            "lchflags(3) {flag} {}",
            shell::quote(&self.path.to_string_lossy())
        ))
    }

    fn relaunches(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Parse the table below the top-level `filesystem` key.
pub fn parse(value: &toml::Value) -> Result<Vec<Setting>> {
    let paths = value.as_table().context("filesystem must be a table")?;
    let mut settings = Vec::new();

    for (path, value) in paths {
        if path.is_empty() {
            bail!("filesystem path may not be empty");
        }

        let table = value.as_table().with_context(|| {
            format!("filesystem path {path:?} must be a table")
        })?;

        for key in table.keys() {
            if key != "hidden" {
                bail!("filesystem.{path:?}.{key} is unsupported");
            }
        }

        let hidden = table
            .get("hidden")
            .with_context(|| format!("filesystem.{path:?}.hidden is required"))?
            .as_bool()
            .with_context(|| {
                format!("filesystem.{path:?}.hidden must be a boolean")
            })?;

        settings.push(Setting { path: PathBuf::from(path), hidden });
    }

    Ok(settings)
}

/// Read current state and build an owned reconciliation plan.
pub fn plan(settings: &[Setting]) -> Result<Vec<Change>> {
    settings
        .iter()
        .map(|setting| {
            let path = resolve(&setting.path)?;
            let current = is_hidden(&path)?;
            Ok(Change { path, desired: setting.hidden, current })
        })
        .collect()
}

/// Get the aboslute path of the dir and also ensure it exists
fn resolve(configured: &Path) -> Result<PathBuf> {
    let path = expand_home(configured)?;
    if !path.is_absolute() {
        bail!(
            "filesystem path must be absolute or begin with '~': {}",
            configured.display()
        );
    }

    fs::symlink_metadata(&path)
        .with_context(|| format!("could not inspect {}", path.display()))?;

    Ok(path)
}

/// Check if a file is hidden
fn is_hidden(path: &Path) -> Result<bool> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;

    Ok(metadata.st_flags() & UF_HIDDEN != 0)
}

/// Hide the file
fn set_hidden(path: &Path, hidden: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;

    // get the current flags and toggle UF_HIDDEN as requested
    let current = metadata.st_flags();
    let desired =
        if hidden { current | UF_HIDDEN } else { current & !UF_HIDDEN };

    // convert the filename to a C string so we can use the libc call
    let path_bytes = path.as_os_str().as_bytes();
    let path_c = CString::new(path_bytes)
        .context("filesystem path contains a null byte")?;

    // set the new flags
    let result = unsafe { lchflags(path_c.as_ptr(), desired) };
    if result == -1 {
        return Err(std::io::Error::last_os_error()).with_context(|| {
            format!("could not change flags on {}", path.display())
        });
    }

    Ok(())
}
