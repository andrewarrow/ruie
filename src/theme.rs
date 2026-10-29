use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, px, rgb};

/// Quiet native surfaces, with the familiar HN orange reserved for emphasis.
pub fn install(cx: &mut App, dark: bool) {
    Theme::change(if dark { ThemeMode::Dark } else { ThemeMode::Light }, None, cx);
    Theme::update(cx, |theme| {
        theme.font_size = px(14.);
        theme.mono_font_size = px(13.);
        let (background, foreground, sidebar, border, muted, secondary, accent, primary) = if dark {
            (0x18191b, 0xe9e9e7, 0x141517, 0x2c2d30, 0x242528, 0xa4a5a8, 0x302720, 0xf59454)
        } else {
            (0xfefdfb, 0x252522, 0xf5f4f0, 0xe1e0da, 0xefeee9, 0x66655f, 0xfff0e3, 0xa8460b)
        };
        theme.background = rgb(background).into();
        theme.foreground = rgb(foreground).into();
        theme.sidebar = rgb(sidebar).into();
        theme.sidebar_foreground = theme.foreground;
        theme.title_bar = theme.sidebar;
        theme.border = rgb(border).into();
        theme.title_bar_border = theme.border;
        theme.sidebar_border = theme.border;
        theme.status_bar = theme.sidebar;
        theme.status_bar_border = theme.border;
        theme.muted = rgb(muted).into();
        theme.muted_foreground = rgb(secondary).into();
        theme.primary = rgb(primary).into();
        theme.primary_foreground = rgb(if dark { 0x23170d } else { 0xffffff }).into();
        theme.accent = rgb(accent).into();
        theme.accent_foreground = theme.foreground;
        theme.list_active = theme.accent;
        theme.list_active_border = theme.primary;
        theme.list_hover = theme.muted;
        theme.input = theme.border;
        theme.ring = theme.primary;
    });
}
