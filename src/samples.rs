pub const SOURCE: &str = r#"use gpui_kit::component::{
    button::Button, dock::DockLayout,
    h_flex, v_flex, ActiveTheme,
};
use gpui_kit::*;

/// The workspace is a composition of dockable views.
struct Workspace {
    navigator: Entity<Navigator>,
    canvas: Entity<Canvas>,
    inspector: Entity<Inspector>,
}

impl Render for Workspace {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .size_full()
            .bg(theme.background)
            .child(self.toolbar(window, cx))
            .child(
                h_flex()
                    .flex_1()
                    .items_stretch()
                    .child(self.navigator.clone())
                    .child(self.canvas.clone())
                    .child(self.inspector.clone()),
            )
            .child(self.status_bar(cx))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        // Native windows. GPU-rendered controls.
        open_workspace(cx);
    });
}
"#;

pub const THEME: &str = r#"[workspace]
name = "Meridian"
appearance = "dark"

[editor]
font_family = "SF Mono"
font_size = 13
line_numbers = true
folding = true

[layout]
navigator = "left"
inspector = "right"
debug_area = "bottom"
draggable_tabs = true
resizable_panes = true

[preview]
device = "Desktop"
scale = 1.0
show_bounds = true
"#;

pub fn project_tree() -> Vec<gpui_kit::component::tree::TreeItem> {
    use gpui_kit::component::tree::TreeItem;
    vec![
        TreeItem::new("meridian", "Meridian")
            .expanded(true)
            .children([
                TreeItem::new("src", "src").expanded(true).children([
                    TreeItem::new("src/main.rs", "main.rs"),
                    TreeItem::new("src/workspace", "workspace")
                        .expanded(true)
                        .children([
                            TreeItem::new("src/workspace/mod.rs", "mod.rs"),
                            TreeItem::new("src/workspace/canvas.rs", "canvas.rs"),
                            TreeItem::new("src/workspace/toolbar.rs", "toolbar.rs"),
                            TreeItem::new("src/workspace/status.rs", "status.rs"),
                        ]),
                    TreeItem::new("src/components", "components")
                        .expanded(true)
                        .children([
                            TreeItem::new("src/components/track.rs", "track.rs"),
                            TreeItem::new("src/components/player.rs", "player.rs"),
                            TreeItem::new("src/components/waveform.rs", "waveform.rs"),
                        ]),
                    TreeItem::new("src/theme.rs", "theme.rs"),
                ]),
                TreeItem::new("assets", "assets").children([
                    TreeItem::new("assets/icons", "icons"),
                    TreeItem::new("assets/fonts", "fonts"),
                ]),
                TreeItem::new("tests", "tests")
                    .children([TreeItem::new("tests/workspace.rs", "workspace.rs")]),
                TreeItem::new("Cargo.toml", "Cargo.toml"),
                TreeItem::new("theme.toml", "theme.toml"),
                TreeItem::new("README.md", "README.md"),
            ]),
    ]
}
