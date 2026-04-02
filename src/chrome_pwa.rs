//! Chrome/Chromium installed PWA: `app_id` from the compositor is per-window; `/proc/<pid>/cmdline`
//! may be shared across windows and carry the wrong `--app-id=`. Align argv with `app_id`.

use std::path::Path;

/// Opaque id between `chrome-` / `chromium-` and the profile segment, e.g. `nabc…32…` in
/// `chrome-nabc…-Default`.
pub fn site_id_from_chromium_style_app_id(app_id: &str) -> Option<&str> {
    let rest = app_id
        .strip_prefix("chrome-")
        .or_else(|| app_id.strip_prefix("chromium-"))?;
    let i = rest.find('-')?;
    let id = &rest[..i];
    if id.len() >= 20 && id.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(id)
    } else {
        None
    }
}

fn looks_like_chrome_executable(bin: &str) -> bool {
    let base = Path::new(bin)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(bin);
    matches!(
        base,
        "chrome" | "google-chrome" | "google-chrome-stable" | "chromium" | "chromium-browser"
    ) || bin.contains("/chrome/")
}

/// Sets or corrects `--app-id=…` from Wayland `app_id` when it encodes a PWA site id.
pub fn align_chrome_pwa_argv(app_id: Option<&str>, command: &mut Vec<String>) {
    let Some(aid) = app_id else {
        return;
    };
    let Some(expected) = site_id_from_chromium_style_app_id(aid) else {
        return;
    };

    for arg in command.iter_mut() {
        if let Some(cur) = arg.strip_prefix("--app-id=") {
            if cur != expected {
                *arg = format!("--app-id={expected}");
            }
            return;
        }
    }

    if command
        .first()
        .map(|p| looks_like_chrome_executable(p))
        .unwrap_or(false)
    {
        command.push(format!("--app-id={expected}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_site_id_default_profile() {
        assert_eq!(
            site_id_from_chromium_style_app_id(
                "chrome-nabcfloaenlmmbnehgecdlmmhienenon-Default"
            ),
            Some("nabcfloaenlmmbnehgecdlmmhienenon")
        );
    }

    #[test]
    fn parses_site_id_youtube_pwa() {
        assert_eq!(
            site_id_from_chromium_style_app_id(
                "chrome-cinhimbnkkaeohfgghhklpknlkffjgod-Default"
            ),
            Some("cinhimbnkkaeohfgghhklpknlkffjgod")
        );
    }

    #[test]
    fn align_fixes_wrong_app_id() {
        let mut cmd = vec![
            "/opt/google/chrome/chrome".into(),
            "--profile-directory=Default".into(),
            "--app-id=cinhimbnkkaeohfgghhklpknlkffjgod".into(),
        ];
        align_chrome_pwa_argv(
            Some("chrome-nabcfloaenlmmbnehgecdlmmhienenon-Default"),
            &mut cmd,
        );
        assert_eq!(
            cmd[2],
            "--app-id=nabcfloaenlmmbnehgecdlmmhienenon"
        );
    }

    #[test]
    fn align_idempotent_when_matching() {
        let mut cmd = vec![
            "/opt/google/chrome/chrome".into(),
            "--app-id=cinhimbnkkaeohfgghhklpknlkffjgod".into(),
        ];
        align_chrome_pwa_argv(
            Some("chrome-cinhimbnkkaeohfgghhklpknlkffjgod-Default"),
            &mut cmd,
        );
        assert_eq!(cmd[1], "--app-id=cinhimbnkkaeohfgghhklpknlkffjgod");
    }
}
