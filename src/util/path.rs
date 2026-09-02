/*!
 * Path helper functions
 */

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// expand a leading `~` path component to the current user's home directory.
pub fn expand_home(path: &Path) -> Result<PathBuf> {
    let mut components = path.components();

    let Some(first) = components.next() else {
        bail!("path may not be empty");
    };

    if first.as_os_str() != "~" {
        return Ok(path.to_owned());
    }

    let home = env::home_dir().context("could not find home directory")?;
    Ok(components.fold(home, |path, component| path.join(component)))
}

/// make a path "friendly" by translating the home directory to "~"
pub fn friendly_path(path: &Path) -> String {
    let Some(home) = env::home_dir() else {
        return path.display().to_string();
    };

    let home = Path::new(&home);
    match path.strip_prefix(home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}
