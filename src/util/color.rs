/*!
 * ANSI color helpers
 */

/// Colorize a string
pub fn colorize<T: ToString>(text: &str, style: T, enabled: bool) -> String {
    let style = style.to_string();
    if enabled {
        format!("\x1b[{}m{}\x1b[0m", style, text)
    } else {
        text.to_owned()
    }
}
