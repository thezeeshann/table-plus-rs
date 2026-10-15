use serde::Serialize;

/// Theme requests made when the app was launched. Validation and precedence
/// (flag, env, saved setting, default) live in the frontend's `lib/theme.ts`,
/// next to the theme table, so the two can never disagree about valid ids.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct LaunchTheme {
    /// From `--theme NAME` or `--theme=NAME`.
    pub flag: Option<String>,
    /// From `$TABLE_PLUS_RS_THEME`.
    pub env: Option<String>,
}

fn from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == "--theme" {
            return it.next();
        }
        if let Some(v) = a.strip_prefix("--theme=") {
            return Some(v.to_string());
        }
    }
    None
}

#[tauri::command]
pub fn launch_theme() -> LaunchTheme {
    LaunchTheme { flag: from_args(std::env::args().skip(1)), env: std::env::var("TABLE_PLUS_RS_THEME").ok() }
}

#[cfg(test)]
mod tests {
    use super::from_args;

    fn args(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn parses_both_flag_forms() {
        assert_eq!(from_args(args(&["--theme", "atelier"])), Some("atelier".into()));
        assert_eq!(from_args(args(&["x", "--theme=one-dark"])), Some("one-dark".into()));
        assert_eq!(from_args(args(&["--other"])), None);
        assert_eq!(from_args(args(&["--theme"])), None);
    }
}
