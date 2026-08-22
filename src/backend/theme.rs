use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ThemeColors {
    pub(super) background: String,
    pub(super) foreground: String,
    pub(super) accent: String,
    pub(super) selection: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            background: "#101010".to_owned(),
            foreground: "#eeeeee".to_owned(),
            accent: "#5584aa".to_owned(),
            selection: "#263746".to_owned(),
        }
    }
}

pub(super) fn omarchy_current_theme_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".local/state/omarchy/current/theme"))
}

pub(super) fn load_omarchy_theme() -> ThemeColors {
    let fallback = ThemeColors::default();
    let Some(path) = omarchy_current_theme_path() else {
        return fallback;
    };
    let Ok(contents) = std::fs::read_to_string(path.join("colors.toml")) else {
        return fallback;
    };
    parse_theme_colors(&contents, fallback)
}

fn parse_theme_colors(contents: &str, mut colors: ThemeColors) -> ThemeColors {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value
            .trim()
            .trim_matches(|character| character == '\"' || character == '\'')
            .to_owned();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "background" => colors.background = value,
            "foreground" => colors.foreground = value,
            "accent" => colors.accent = value,
            "selection" => colors.selection = value,
            _ => {}
        }
    }
    colors
}

#[cfg(test)]
mod tests {
    use super::{ThemeColors, parse_theme_colors};

    #[test]
    fn parses_the_omarchy_color_contract() {
        let colors = parse_theme_colors(
            "mode = 'dark'\nbackground = '#111c18'\nforeground = \"#C1C497\"\naccent = '#509475'\nselection = '#32473B'\n",
            ThemeColors::default(),
        );
        assert_eq!(colors.background, "#111c18");
        assert_eq!(colors.foreground, "#C1C497");
        assert_eq!(colors.accent, "#509475");
        assert_eq!(colors.selection, "#32473B");
    }
}
