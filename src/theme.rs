use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, px, rgb};

/// Product palette lives here; views use the kit's semantic roles.
pub fn install(cx: &mut App, dark: bool) {
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    Theme::update(cx, |theme| {
        theme.font_size = px(14.);
        theme.mono_font_size = px(13.);
        if dark {
            theme.background = rgb(0x181b22).into();
            theme.foreground = rgb(0xd9dee9).into();
            theme.sidebar = rgb(0x20242d).into();
            theme.sidebar_foreground = theme.foreground;
            theme.title_bar = rgb(0x252a34).into();
            theme.tab_bar = theme.sidebar;
            theme.tab = theme.sidebar;
            theme.tab_active = theme.background;
            theme.tab_active_foreground = theme.foreground;
            theme.border = rgb(0x343a46).into();
            theme.title_bar_border = theme.border;
            theme.sidebar_border = theme.border;
            theme.status_bar = theme.sidebar;
            theme.status_bar_border = theme.border;
            theme.muted = rgb(0x282e3a).into();
            theme.muted_foreground = rgb(0x939fb3).into();
            theme.primary = rgb(0x8aafff).into();
            theme.primary_foreground = rgb(0x151d2e).into();
            theme.accent = rgb(0x303d56).into();
            theme.accent_foreground = theme.foreground;
            theme.list_active = theme.accent;
            theme.list_active_border = theme.primary;
            theme.list_hover = theme.muted;
            theme.input = theme.border;
        }
    });
}
