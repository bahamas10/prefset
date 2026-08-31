/*!
 * Path helper functions
 */

use std::env;
use std::path::Path;

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
