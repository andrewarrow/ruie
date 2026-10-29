use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, StyledExt, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    dock::{DockArea, DockLayout, DockPlacement, DockSkin, PanelStyle, panel_handle},
    h_flex,
    menu::{DropdownMenu, PopupMenuItem},
    status_bar::StatusBar,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder, *};

use crate::{
    panels::{DemoPanel, Surface},
    *,
};

#[cfg(feature = "visual-check")]
#[path = "visual_check.rs"]
pub(crate) mod visual_check;

pub fn open(cx: &mut App, controls_window: bool) {
    // Window bounds are physical platform geometry. Pane sizes below follow rem.
    let window_size = if controls_window {
        size(px(960.), px(720.))
    } else {
        size(px(1440.), px(900.))
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            window_size,
            cx,
        ))),
        window_min_size: Some(if controls_window {
            size(px(720.), px(560.))
        } else {
            size(px(980.), px(640.))
        }),
        ..TitleBar::window_options()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title(if controls_window {
            "Ruie Studio — Controls"
        } else {
            "Ruie Studio — Meridian"
        });
        cx.new(|cx| Workspace::new(controls_window, window, cx))
    })
    .expect("could not open the Ruie Studio window");
}

struct Workspace {
    dock: Entity<DockArea>,
    skin: Rc<DockSkin>,
    focus: FocusHandle,
    controls_window: bool,
    locked: bool,
}

impl Workspace {
    fn new(controls_window: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (dock, skin) = DockSkin::dock_area("ruie-workspace", Some(1), window, cx);
        skin.set_panel_style(PanelStyle::TabBar, cx);
        skin.set_close_button_visible(true, cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut this = Self {
            dock,
            skin,
            focus,
            controls_window,
            locked: false,
        };
        this.reset(window, cx);
        this
    }

    fn tabs(surfaces: &[Surface], window: &mut Window, cx: &mut App) -> DockLayout {
        surfaces.iter().fold(DockLayout::tabs(), |layout, surface| {
            layout.panel_view(panel_handle(DemoPanel::new(*surface, window, cx)), cx)
        })
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use Surface::*;
        // Resolve the pane defaults against the same scale as their content.
        let rem = window.rem_size();
        let center = if self.controls_window {
            DockLayout::h_split()
                .child(Self::tabs(&[Controls], window, cx), None)
                .child(Self::tabs(&[Inspector], window, cx), None)
                .child(Self::tabs(&[Preview], window, cx), None)
        } else {
            DockLayout::h_split()
                .child(Self::tabs(&[Source, Theme], window, cx), None)
                .child(Self::tabs(&[Preview], window, cx), Some(rem * 23.))
        };
        let left = DockLayout::v_split()
            .child(Self::tabs(&[Project, Search], window, cx), None)
            .child(Self::tabs(&[Outline], window, cx), Some(rem * 15.));
        let right = Self::tabs(&[Inspector, Controls], window, cx);
        let bottom = DockLayout::h_split()
            .child(Self::tabs(&[Console, Problems], window, cx), None)
            .child(
                Self::tabs(&[Variables, Threads], window, cx),
                Some(rem * 27.),
            )
            .child(Self::tabs(&[Performance], window, cx), Some(rem * 18.));
        self.dock.update(cx, |area, cx| {
            area.set_zoomed_out(window, cx);
            area.set_locked(false, window, cx);
            area.set_center(center, window, cx);
            if !self.controls_window {
                area.set_dock(DockPlacement::Left, left, window, cx);
                area.set_dock_size(DockPlacement::Left, rem * 16., window, cx);
                area.set_dock(DockPlacement::Right, right, window, cx);
                area.set_dock_size(DockPlacement::Right, rem * 19., window, cx);
                area.set_dock(DockPlacement::Bottom, bottom, window, cx);
                area.set_dock_size(DockPlacement::Bottom, rem * 16., window, cx);
                for placement in [
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ] {
                    area.set_dock_collapsible(placement, true, window, cx);
                }
            }
        });
        self.locked = false;
        self.skin.set_close_button_visible(true, cx);
        cx.notify();
    }

    fn toggle(&mut self, placement: DockPlacement, window: &mut Window, cx: &mut Context<Self>) {
        self.dock
            .update(cx, |area, cx| area.toggle_dock(placement, window, cx));
        cx.notify();
    }

    fn toolbar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = window.viewport_size().width < window.rem_size() * 85.;
        let locked = self.locked;
        let owner = cx.weak_entity();
        h_flex()
            .h_11()
            .px_3()
            .gap_2()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                Icon::new(IconName::Box)
                    .small()
                    .text_color(cx.theme().primary),
            )
            .child(div().font_semibold().child(if self.controls_window {
                "Controls lab"
            } else {
                "Meridian"
            }))
            .when(!compact, |el| {
                el.child(
                    h_flex()
                        .ml_3()
                        .gap_2()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(IconName::GitBranch).xsmall())
                        .child("main")
                        .child("/")
                        .child("workspace"),
                )
            })
            .child(div().flex_1())
            .when(!compact, |el| {
                el.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .mr_2()
                        .child("UI showcase"),
                )
            })
            .child(
                Button::new("controls-window")
                    .small()
                    .ghost()
                    .icon(IconName::AppWindow)
                    .label(if compact {
                        "Controls…"
                    } else {
                        "Controls window…"
                    })
                    .on_click(|_, _, cx| open(cx, true)),
            )
            .child(
                Button::new("layout-menu")
                    .small()
                    .ghost()
                    .icon(IconName::PanelsTopLeft)
                    .label("Layout")
                    .dropdown_caret(true)
                    .dropdown_menu(move |menu, _, _| {
                        crate::layout_menu(menu).separator().item(
                            PopupMenuItem::new("Lock layout").checked(locked).on_click({
                                let owner = owner.clone();
                                move |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.locked = !this.locked;
                                        this.dock.update(cx, |area, cx| {
                                            area.set_locked(this.locked, window, cx)
                                        });
                                        cx.notify();
                                    });
                                }
                            }),
                        )
                    }),
            )
            .child(
                Button::new("appearance")
                    .small()
                    .ghost()
                    .icon(if cx.theme().mode == ThemeMode::Dark {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    })
                    .accessibility_label("Toggle appearance")
                    .tooltip("Toggle appearance · ⌘⇧T")
                    .on_click(|_, _, cx| {
                        crate::theme::install(cx, cx.theme().mode != ThemeMode::Dark)
                    }),
            )
            .child(
                Button::new("about")
                    .small()
                    .ghost()
                    .icon(IconName::CircleQuestionMark)
                    .accessibility_label("About this showcase")
                    .tooltip("About this showcase")
                    .on_click(|_, window, cx| crate::about(window, cx)),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("workspace")
            .track_focus(&self.focus)
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .on_action(cx.listener(|this, _: &ResetLayout, window, cx| this.reset(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleLeft, window, cx| {
                this.toggle(DockPlacement::Left, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleRight, window, cx| {
                this.toggle(DockPlacement::Right, window, cx)
            }))
            .on_action(cx.listener(|this, _: &ToggleBottom, window, cx| {
                this.toggle(DockPlacement::Bottom, window, cx)
            }))
            .on_action(|_: &ToggleTheme, _, cx| {
                crate::theme::install(cx, cx.theme().mode != ThemeMode::Dark)
            })
            .on_action(|_: &NewWindow, _, cx| open(cx, true))
            .on_action(|_: &About, window, cx| crate::about(window, cx))
            .child(
                TitleBar::new().child(
                    h_flex()
                        .justify_between()
                        .w_full()
                        .pr_3()
                        .child(div().text_xs().font_semibold().child("Ruie Studio"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("GPUI Kit · Native Rust"),
                        ),
                ),
            )
            .child(self.toolbar(window, cx))
            .child(div().flex_1().min_h_0().min_w_0().child(self.dock.clone()))
            .child(
                StatusBar::new()
                    .left(
                        Button::new("left-dock")
                            .xsmall()
                            .ghost()
                            .icon(IconName::PanelLeft)
                            .accessibility_label("Toggle navigator")
                            .tooltip("Navigator · ⌘1")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle(DockPlacement::Left, window, cx)
                            })),
                    )
                    .left(
                        Button::new("bottom-dock")
                            .xsmall()
                            .ghost()
                            .icon(IconName::PanelBottom)
                            .accessibility_label("Toggle debug area")
                            .tooltip("Debug area · ⌘2")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle(DockPlacement::Bottom, window, cx)
                            })),
                    )
                    .left(
                        Button::new("right-dock")
                            .xsmall()
                            .ghost()
                            .icon(IconName::PanelRight)
                            .accessibility_label("Toggle inspector")
                            .tooltip("Inspector · ⌘3")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle(DockPlacement::Right, window, cx)
                            })),
                    )
                    .left("Drag tabs to rearrange · Resize panel edges")
                    .right("Rust")
                    .right("UTF-8")
                    .right("UI only")
                    .child(
                        Button::new("reset-layout")
                            .xsmall()
                            .ghost()
                            .label("Reset layout")
                            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                    ),
            )
    }
}
