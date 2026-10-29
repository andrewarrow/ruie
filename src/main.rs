mod panels;
mod samples;
mod theme;
mod workspace;

use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::*;

actions!(
    ruie,
    [
        Quit,
        ResetLayout,
        ToggleLeft,
        ToggleRight,
        ToggleBottom,
        ToggleTheme,
        NewWindow,
        About
    ]
);

fn main() {
    #[cfg(feature = "visual-check")]
    if std::env::var_os("RUIE_VISUAL_CHECK").is_some() {
        workspace::visual_check::run();
        return;
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::install(cx, true);
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-1", ToggleLeft, None),
                KeyBinding::new("cmd-2", ToggleBottom, None),
                KeyBinding::new("cmd-3", ToggleRight, None),
                KeyBinding::new("cmd-shift-0", ResetLayout, None),
                KeyBinding::new("cmd-shift-n", NewWindow, None),
                KeyBinding::new("cmd-shift-t", ToggleTheme, None),
            ]);
            cx.set_menus(vec![
                Menu {
                    name: "Ruie Studio".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("About Ruie Studio…", About),
                        MenuItem::separator(),
                        MenuItem::action("Quit", Quit),
                    ],
                },
                Menu {
                    name: "View".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Toggle Navigator", ToggleLeft),
                        MenuItem::action("Toggle Debug Area", ToggleBottom),
                        MenuItem::action("Toggle Inspector", ToggleRight),
                        MenuItem::separator(),
                        MenuItem::action("Reset Layout", ResetLayout),
                        MenuItem::action("Toggle Appearance", ToggleTheme),
                    ],
                },
                Menu {
                    name: "Window".into(),
                    disabled: false,
                    items: vec![MenuItem::action("Controls Window…", NewWindow)],
                },
            ]);
            workspace::open(cx, false);
            cx.activate(true);
        });
}

fn about(window: &mut Window, cx: &mut App) {
    use gpui_kit::component::{WindowExt, v_flex};
    window.open_dialog(cx, |dialog, _, _| {
        dialog.title("Ruie Studio").child(v_flex().gap_3()
            .child("A native Rust UI showcase built with GPUI Kit.")
            .child("Drag tabs between groups or to a panel edge. Resize dividers, zoom panels from their menus, or open a separate controls window.")
            .child("All files, logs, diagnostics, and measurements are sample content. Controls change only local UI state."))
    });
}

fn layout_menu(menu: gpui_kit::component::menu::PopupMenu) -> gpui_kit::component::menu::PopupMenu {
    menu.item(PopupMenuItem::new("Navigator").action(Box::new(ToggleLeft)))
        .item(PopupMenuItem::new("Debug area").action(Box::new(ToggleBottom)))
        .item(PopupMenuItem::new("Inspector").action(Box::new(ToggleRight)))
        .separator()
        .item(PopupMenuItem::new("Reset layout").action(Box::new(ResetLayout)))
}
