//! Product names for the Host's `open-in-app` catalog ids.
//!
//! The reference names only the ids it can label and hides every other probe result
//! (`ui-open-in-app/src/client/applications.ts:1-40`); native does the same rather than showing a
//! raw catalog id. The catalog arrives from the Host's plain `GET /open-in-app/apps` route.
/// The English product name for one catalog id, or `None` when the reference names none.
pub fn label(id: &str) -> Option<&'static str> {
    Some(match id {
        "finder" => "Finder",
        "explorer" => "File Explorer",
        "filemanager" => "Files",
        "cursor" => "Cursor",
        "vscode" => "VS Code",
        "vscodeinsiders" => "VS Code Insiders",
        "windsurf" => "Windsurf",
        "zed" => "Zed",
        "sublimetext" => "Sublime Text",
        "xcode" => "Xcode",
        "androidstudio" => "Android Studio",
        "intellij" => "IntelliJ IDEA",
        "pycharm" => "PyCharm",
        "webstorm" => "WebStorm",
        "phpstorm" => "PhpStorm",
        "goland" => "GoLand",
        "rider" => "Rider",
        "rustrover" => "RustRover",
        "fork" => "Fork",
        "sourcetree" => "Sourcetree",
        "github" => "GitHub Desktop",
        "tower" => "Tower",
        "gitkraken" => "GitKraken",
        "smartgit" => "SmartGit",
        "sublimemerge" => "Sublime Merge",
        "ghostty" => "Ghostty",
        "warp" => "Warp",
        "iterm" => "iTerm2",
        "kitty" => "kitty",
        "terminal" => "Terminal",
        "windowsterminal" => "Windows Terminal",
        "gitbash" => "Git Bash",
        "gnometerminal" => "GNOME Terminal",
        "konsole" => "Konsole",
        _ => return None,
    })
}

/// The nameable installed applications in Host menu order, with the remembered choice first when it
/// is still installed (`controller.ts:47-62`: a stale choice falls back to the first nameable app).
pub fn nameable<'a>(apps: &'a [String], choice: Option<&str>) -> Vec<(&'a str, &'static str)> {
    let named: Vec<(&str, &'static str)> = apps
        .iter()
        .filter_map(|id| label(id).map(|name| (id.as_str(), name)))
        .collect();
    let Some(choice) = choice else {
        return named;
    };
    match named.iter().position(|(id, _)| *id == choice) {
        Some(index) => {
            let mut ordered = named;
            let chosen = ordered.remove(index);
            ordered.insert(0, chosen);
            ordered
        }
        None => named,
    }
}

/// The Session's own working directory, which is what the opener launches.
pub fn workspace_of(cwd: Option<&str>) -> Option<&str> {
    cwd.filter(|path| path.starts_with('/') && !path.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_named_ids_survive_and_the_choice_leads_when_it_is_installed() {
        assert_eq!(label("vscode"), Some("VS Code"));
        assert_eq!(label("VS Code"), None);
        assert_eq!(label("not-an-app"), None);
        let apps = vec![
            "vscode".to_owned(),
            "unknown".to_owned(),
            "kitty".to_owned(),
        ];
        assert_eq!(
            nameable(&apps, Some("kitty")),
            [("kitty", "kitty"), ("vscode", "VS Code")]
        );
        // A remembered app that is gone falls back to the Host's order, never to a missing row.
        assert_eq!(
            nameable(&apps, Some("gone")),
            [("vscode", "VS Code"), ("kitty", "kitty")]
        );
        assert!(nameable(&["unknown".to_owned()], None).is_empty());
        assert_eq!(
            workspace_of(Some("/PUBLIC/workspace")),
            Some("/PUBLIC/workspace")
        );
        assert_eq!(workspace_of(Some("relative/path")), None);
        assert_eq!(workspace_of(Some("")), None);
        assert_eq!(workspace_of(None), None);
    }
}
