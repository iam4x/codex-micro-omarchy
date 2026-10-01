use gpui::{AssetSource, SharedString, px, rgb};
use gpui_component::{Theme, ThemeMode};
use std::{borrow::Cow, fs, path::PathBuf};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: u32,
    pub panel: u32,
    pub raised: u32,
    pub border: u32,
    pub text: u32,
    pub muted: u32,
    pub accent: u32,
    pub green: u32,
    pub red: u32,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            background: 0x141414,
            panel: 0x0f0f0f,
            raised: 0x242424,
            border: 0x303030,
            text: 0xd4d4d4,
            muted: 0x838383,
            accent: 0x81a1c1,
            green: 0x70b489,
            red: 0xfc6b83,
        }
    }
}
impl Palette {
    pub fn load() -> Self {
        let mut palette = Self::default();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let path = state.join("omarchy/current/theme/colors.toml");
        if let Ok(source) = fs::read_to_string(path)
            && let Ok(colors) = toml::from_str::<toml::Table>(&source)
        {
            let read = |name: &str, fallback: u32| {
                colors
                    .get(name)
                    .and_then(toml::Value::as_str)
                    .and_then(parse_color)
                    .unwrap_or(fallback)
            };
            palette.background = read("background", palette.background);
            palette.panel = read("dark_background", palette.panel);
            palette.raised = read("lighter_background", palette.raised);
            palette.text = read("foreground", palette.text);
            palette.accent = read("accent", palette.accent);
            palette.green = read("bright_green", palette.green);
            palette.red = read("red", palette.red);
        }
        palette
    }
    pub fn apply(self, cx: &mut gpui::App) {
        Theme::change(ThemeMode::Dark, None, cx);
        let theme = Theme::global_mut(cx);
        theme.colors.background = rgb(self.background).into();
        theme.colors.foreground = rgb(self.text).into();
        theme.colors.input = rgb(self.panel).into();
        theme.colors.border = rgb(self.border).into();
        theme.colors.muted_foreground = rgb(self.muted).into();
        theme.colors.primary = rgb(self.accent).into();
        theme.colors.ring = rgb(self.accent).into();
        theme.colors.caret = rgb(self.accent).into();
        theme.colors.selection = rgb(self.raised).into();
        theme.colors.danger = rgb(self.red).into();
        theme.colors.danger_hover = rgb(self.red).into();
        theme.colors.danger_active = rgb(self.red).into();
        theme.colors.danger_foreground = rgb(self.panel).into();
        theme.colors.overlay = gpui::hsla(0., 0., 0., 0.55);
        theme.font_family = "Inter".into();
        theme.font_size = px(14.);
        theme.radius = px(3.);
        theme.radius_lg = px(4.);
        theme.shadow = false;
    }
}

fn parse_color(value: &str) -> Option<u32> {
    let value = value.strip_prefix('#').unwrap_or(value);
    (value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

pub struct Assets;
include!(concat!(env!("OUT_DIR"), "/icons.rs"));
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICON_ASSETS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }
    fn list(&self, prefix: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICON_ASSETS
            .iter()
            .filter(|(name, _)| name.starts_with(prefix))
            .map(|(name, _)| (*name).into())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn colors_require_six_rgb_hex_digits() {
        assert_eq!(super::parse_color("#81a1c1"), Some(0x81a1c1));
        assert_eq!(super::parse_color("FFFFFF"), Some(0xffffff));
        for value in ["#fff", "", "#12345678", "##123456", "#gg0000", "#-12345"] {
            assert_eq!(super::parse_color(value), None, "{value}");
        }
    }
}
