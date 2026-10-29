//! Optional native GPU snapshots and a small smoke check for the showcase shell.
use std::{path::Path, sync::Arc};

use gpui_kit::{
    AppContext, Bounds, HeadlessAppContext, WindowBounds, WindowOptions,
    component::{Theme, dock::DockPlacement},
    px, size,
    test::TestWindowExt,
};

use super::Workspace;

pub(crate) fn run() {
    let output = std::env::var("RUIE_VISUAL_DIR").unwrap_or_else(|_| "/tmp/ruie-visuals".into());
    std::fs::create_dir_all(&output).expect("create snapshot directory");
    let mut app = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    app.update(gpui_kit::init);
    app.update(|cx| crate::theme::install(cx, true));

    for (name, width, height, dark, controls, zoom) in [
        ("studio-1440", 1440., 900., true, false, 14.),
        ("studio-1280", 1280., 800., true, false, 14.),
        ("studio-minimum", 980., 640., true, false, 14.),
        ("studio-light", 1440., 900., false, false, 14.),
        ("controls", 960., 720., true, true, 14.),
        ("studio-zoom", 1440., 900., true, false, 16.),
    ] {
        app.update(|cx| {
            crate::theme::install(cx, dark);
            Theme::update(cx, |theme| theme.font_size = px(zoom));
        });
        let (handle, workspace) = app.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(width), px(height)),
                    })),
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| Workspace::new(controls, window, cx)),
            )
            .expect("open offscreen workspace")
        });
        app.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        let image = app
            .capture_screenshot(handle)
            .expect("render Metal snapshot");
        image
            .save(Path::new(&output).join(format!("{name}.png")))
            .expect("save snapshot");

        if name == "studio-1440" {
            app.update_window(handle, |_, window, cx| {
                for (button, placement) in [
                    ("left-dock", DockPlacement::Left),
                    ("bottom-dock", DockPlacement::Bottom),
                    ("right-dock", DockPlacement::Right),
                ] {
                    assert!(workspace.read(cx).dock.read(cx).is_dock_open(placement));
                    window.click(button, cx);
                    assert!(!workspace.read(cx).dock.read(cx).is_dock_open(placement));
                    window.click(button, cx);
                    assert!(workspace.read(cx).dock.read(cx).is_dock_open(placement));
                }
                window.click("reset-layout", cx);
                window.render_frame(cx);
                for placement in [
                    DockPlacement::Left,
                    DockPlacement::Bottom,
                    DockPlacement::Right,
                ] {
                    assert!(workspace.read(cx).dock.read(cx).is_dock_open(placement));
                }
                window.click("appearance", cx);
                assert!(!cx.global::<Theme>().is_dark());
                window.click("appearance", cx);
                assert!(cx.global::<Theme>().is_dark());
            })
            .unwrap();
        }
        app.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
        println!("passed {name}");
    }
    println!("Snapshots: {output}");
}
