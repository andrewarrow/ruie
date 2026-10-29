use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, IndexPath, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    chart::LineChart,
    checkbox::Checkbox,
    color_picker::{ColorPicker, ColorPickerState},
    dock::{BasePanel, Panel, PanelEvent},
    h_flex,
    input::{Editor, EditorState, Input, InputState},
    list::ListItem,
    progress::Progress,
    select::{Select, SelectState},
    slider::{Slider, SliderState},
    switch::Switch,
    tab::{Tab, TabBar},
    tree::{TreeState, tree},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder, *};

use crate::samples;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Project,
    Search,
    Outline,
    Source,
    Theme,
    Preview,
    Inspector,
    Controls,
    Console,
    Problems,
    Variables,
    Threads,
    Performance,
}

impl Surface {
    pub fn title(self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Search => "Search",
            Self::Outline => "Symbols",
            Self::Source => "main.rs",
            Self::Theme => "theme.toml",
            Self::Preview => "Canvas",
            Self::Inspector => "Inspector",
            Self::Controls => "Controls",
            Self::Console => "Console",
            Self::Problems => "Problems",
            Self::Variables => "Variables",
            Self::Threads => "Threads",
            Self::Performance => "Performance",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Project => IconName::Folder,
            Self::Search => IconName::Search,
            Self::Outline => IconName::ListTree,
            Self::Source => IconName::FileCode,
            Self::Theme => IconName::FileSliders,
            Self::Preview => IconName::PanelsTopLeft,
            Self::Inspector => IconName::SlidersHorizontal,
            Self::Controls => IconName::Shapes,
            Self::Console => IconName::Terminal,
            Self::Problems => IconName::TriangleAlert,
            Self::Variables => IconName::Braces,
            Self::Threads => IconName::Layers,
            Self::Performance => IconName::Activity,
        }
    }
}

/// Each dockable surface owns only its local demonstration controls.
pub struct DemoPanel {
    surface: Surface,
    focus: FocusHandle,
    tree: Entity<TreeState>,
    input: Entity<InputState>,
    editor: Entity<EditorState>,
    select: Entity<SelectState<Vec<&'static str>>>,
    slider: Entity<SliderState>,
    color: Entity<ColorPickerState>,
    enabled: bool,
    checked: bool,
    segment: usize,
}

impl DemoPanel {
    pub fn new(surface: Surface, window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let tree = cx.new(|cx| {
                let mut state = TreeState::new(cx);
                state.set_items(samples::project_tree(), cx);
                state
            });
            Self {
                surface,
                focus: cx.focus_handle(),
                tree,
                input: cx.new(|cx| {
                    InputState::new(window, cx).placeholder(if surface == Surface::Search {
                        "Find in sample project…"
                    } else {
                        "Filter…"
                    })
                }),
                editor: cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language(if surface == Surface::Theme {
                            "toml"
                        } else {
                            "rust"
                        })
                        .line_number(true)
                        .default_value(if surface == Surface::Theme {
                            samples::THEME
                        } else {
                            samples::SOURCE
                        })
                }),
                select: cx.new(|cx| {
                    SelectState::new(
                        vec!["Automatic", "Horizontal", "Vertical", "Absolute"],
                        Some(IndexPath::default()),
                        window,
                        cx,
                    )
                }),
                slider: cx.new(|_| SliderState::new().min(0.).max(100.).default_value(72.)),
                color: cx
                    .new(|cx| ColorPickerState::new(window, cx).default_value(cx.theme().primary)),
                enabled: true,
                checked: false,
                segment: 0,
            }
        })
    }

    fn navigator(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .h_10()
                    .px_3()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Box)
                            .small()
                            .text_color(cx.theme().primary),
                    )
                    .child(div().font_semibold().child("Meridian"))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("13 files"),
                    ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    tree(&self.tree, |_, entry, _, _, cx| {
                        let icon = if entry.is_folder() {
                            if entry.is_expanded() {
                                IconName::FolderOpen
                            } else {
                                IconName::Folder
                            }
                        } else if entry.item().label.ends_with(".rs") {
                            IconName::FileCode
                        } else {
                            IconName::FileText
                        };
                        ListItem::new(entry.item().id.clone())
                            .h_7()
                            .px_2()
                            .pl(rems(0.75 + entry.depth() as f32 * 0.85))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Icon::new(icon)
                                            .small()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .child(div().text_xs().child(entry.item().label.clone())),
                            )
                    })
                    .size_full(),
                ),
            )
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.input)
                            .small()
                            .prefix(Icon::new(IconName::ListFilter).small()),
                    ),
            )
            .into_any_element()
    }

    fn source(&self, cx: &App) -> AnyElement {
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .h_8()
                    .px_3()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Meridian")
                    .child(Icon::new(IconName::ChevronRight).xsmall())
                    .child("src")
                    .child(Icon::new(IconName::ChevronRight).xsmall())
                    .child(
                        div()
                            .text_color(cx.theme().foreground)
                            .child(self.surface.title()),
                    )
                    .child(div().flex_1())
                    .child("Read only"),
            )
            .child(
                Editor::new(&self.editor)
                    .readonly(true)
                    .bordered(false)
                    .aria_label("Read-only sample source")
                    .size_full(),
            )
            .into_any_element()
    }

    fn outline(&self, cx: &App) -> AnyElement {
        scroll("symbols")
            .bg(cx.theme().sidebar)
            .py_2()
            .children(
                [
                    ("Workspace", "struct", false),
                    ("navigator", "Entity", true),
                    ("canvas", "Entity", true),
                    ("inspector", "Entity", true),
                    ("impl Render", "impl", false),
                    ("render", "fn", true),
                    ("main", "fn", false),
                ]
                .into_iter()
                .map(|(name, kind, indent)| {
                    h_flex()
                        .h_7()
                        .px_3()
                        .gap_2()
                        .when(indent, |el| el.pl_6())
                        .child(
                            Icon::new(if kind == "fn" {
                                IconName::Braces
                            } else {
                                IconName::Box
                            })
                            .xsmall()
                            .text_color(cx.theme().primary),
                        )
                        .child(div().text_xs().child(name))
                        .child(div().flex_1())
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(kind),
                        )
                }),
            )
            .into_any_element()
    }

    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        scroll("inspector")
            .p_3()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Component).small())
                            .child(div().font_semibold().child("Workspace")),
                    )
                    .child(muted("src / workspace / mod.rs", cx)),
            )
            .child(
                section("Identity", cx)
                    .gap_2()
                    .child(property("Type", "Container", cx))
                    .child(property("Element ID", "workspace-root", cx))
                    .child(property("Visibility", "Inherited", cx)),
            )
            .child(
                section("Layout", cx)
                    .gap_3()
                    .child(
                        v_flex().gap_1().child(muted("Direction", cx)).child(
                            Select::new(&self.select)
                                .small()
                                .w_full()
                                .accessibility_label("Layout direction"),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .children([("W", "Fill"), ("H", "Fill")].into_iter().map(
                                |(label, value)| {
                                    h_flex()
                                        .flex_1()
                                        .px_2()
                                        .h_8()
                                        .gap_2()
                                        .border_1()
                                        .border_color(cx.theme().input)
                                        .rounded(cx.theme().radius)
                                        .child(muted(label, cx))
                                        .child(value)
                                },
                            )),
                    )
                    .child(property("Padding", "16   /   16   /   16   /   16", cx))
                    .child(property("Gap", "8 px", cx)),
            )
            .child(
                section("Appearance", cx)
                    .gap_3()
                    .child(
                        h_flex().justify_between().child("Accent").child(
                            ColorPicker::new(&self.color)
                                .small()
                                .label("Color")
                                .accessibility_label("Accent color"),
                        ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .child(muted("Opacity", cx))
                            .child(format!("{:.0}%", self.slider.read(cx).value().start())),
                    )
                    .child(
                        Slider::new(&self.slider)
                            .horizontal()
                            .accessibility_label("Opacity"),
                    )
                    .child(
                        h_flex().justify_between().child("Clip content").child(
                            Switch::new("clip")
                                .small()
                                .checked(self.enabled)
                                .accessibility_label("Clip content")
                                .on_click(cx.listener(|this, value, _, cx| {
                                    this.enabled = *value;
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        Checkbox::new("layout-guides")
                            .label("Show layout guides")
                            .checked(self.checked)
                            .on_click(cx.listener(|this, value, _, cx| {
                                this.checked = *value;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                section("Constraints", cx)
                    .gap_2()
                    .child(property("Minimum width", "320 px", cx))
                    .child(property("Maximum width", "Unbounded", cx))
                    .child(property("Flex grow", "1.0", cx)),
            )
            .into_any_element()
    }

    fn controls(&self, cx: &mut Context<Self>) -> AnyElement {
        scroll("controls")
            .p_4()
            .gap_4()
            .child(
                section("Selection", cx)
                    .gap_3()
                    .child(
                        TabBar::new("alignment")
                            .segmented()
                            .small()
                            .selected_index(self.segment)
                            .children([
                                Tab::new().label("Left"),
                                Tab::new().label("Center"),
                                Tab::new().label("Right"),
                            ])
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.segment = *index;
                                cx.notify();
                            })),
                    )
                    .child(
                        Select::new(&self.select)
                            .small()
                            .w_full()
                            .accessibility_label("Control layout"),
                    )
                    .child(
                        Checkbox::new("control-check")
                            .label("Snap to grid")
                            .checked(self.checked)
                            .on_click(cx.listener(|this, value, _, cx| {
                                this.checked = *value;
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("control-switch")
                            .label("Live preview")
                            .checked(self.enabled)
                            .on_click(cx.listener(|this, value, _, cx| {
                                this.enabled = *value;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                section("Values", cx)
                    .gap_3()
                    .child(
                        Input::new(&self.input)
                            .small()
                            .prefix(Icon::new(IconName::Search).small()),
                    )
                    .child(
                        Slider::new(&self.slider)
                            .horizontal()
                            .accessibility_label("Sample slider"),
                    )
                    .child(property(
                        "Value",
                        &format!("{:.0}", self.slider.read(cx).value().start()),
                        cx,
                    ))
                    .child(
                        ColorPicker::new(&self.color)
                            .label("Accent color")
                            .accessibility_label("Sample color"),
                    ),
            )
            .child(
                section("Progress", cx)
                    .gap_3()
                    .child(Progress::new("sample-progress").value(72.).small())
                    .child(property("Sample assets", "18 / 25", cx)),
            )
            .child(
                section("Button states", cx).gap_2().child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("selected-control")
                                .small()
                                .primary()
                                .label("Selected")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.checked = !this.checked;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("disabled-control")
                                .small()
                                .label("Disabled")
                                .disabled(true),
                        ),
                ),
            )
            .into_any_element()
    }

    fn preview(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .h_9()
                    .px_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Icon::new(IconName::Monitor).small())
                    .child(div().text_xs().child("Desktop"))
                    .child(div().flex_1())
                    .child(muted("1×", cx)),
            )
            .child(
                scroll("canvas")
                    .p_4()
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Player / Default")
                            .child("320 × 400"),
                    )
                    .child(
                        v_flex()
                            .border_1()
                            .border_color(cx.theme().primary)
                            .bg(cx.theme().background)
                            .child(
                                h_flex()
                                    .h_10()
                                    .px_3()
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        Icon::new(IconName::AudioLines)
                                            .small()
                                            .text_color(cx.theme().primary),
                                    )
                                    .child(div().font_semibold().child("Meridian"))
                                    .child(div().flex_1())
                                    .child(Icon::new(IconName::Ellipsis).small()),
                            )
                            .child(
                                v_flex()
                                    .p_4()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("NOW PLAYING"),
                                    )
                                    .child(
                                        div()
                                            .h_24()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .gap_1()
                                            .bg(cx.theme().muted)
                                            .rounded(cx.theme().radius)
                                            .children((0..37).map(|n| {
                                                let height = 0.4
                                                    + ((n as f32 * 0.71).sin().abs()
                                                        * (n as f32 * 0.19).cos().abs())
                                                        * 4.0;
                                                div()
                                                    .w_1()
                                                    .h(rems(height))
                                                    .rounded(cx.theme().radius)
                                                    .bg(if n < 23 {
                                                        cx.theme().primary
                                                    } else {
                                                        cx.theme().border
                                                    })
                                            })),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_semibold()
                                                    .child("A quieter place"),
                                            )
                                            .child(muted("Nils Frahm · Spaces", cx)),
                                    )
                                    .child(Progress::new("playback").value(61.).xsmall())
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .child(muted("3:24", cx))
                                            .child(muted("5:32", cx)),
                                    )
                                    .child(
                                        h_flex()
                                            .justify_center()
                                            .gap_5()
                                            .child(Icon::new(IconName::SkipBack).small())
                                            .child(
                                                div()
                                                    .size_10()
                                                    .rounded_full()
                                                    .bg(cx.theme().primary)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        Icon::new(IconName::Pause).text_color(
                                                            cx.theme().primary_foreground,
                                                        ),
                                                    ),
                                            )
                                            .child(Icon::new(IconName::SkipForward).small()),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(Icon::new(IconName::Volume2).small())
                                            .child(div().flex_1().child(
                                                Progress::new("volume").value(76.).xsmall(),
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        section("Layers", cx).gap_1().children(
                            [
                                ("Player", 0),
                                ("Artwork", 1),
                                ("Track information", 1),
                                ("Transport", 1),
                            ]
                            .into_iter()
                            .map(|(label, depth)| {
                                h_flex()
                                    .h_7()
                                    .gap_2()
                                    .when(depth == 1, |el| el.pl_4())
                                    .child(Icon::new(IconName::Component).xsmall())
                                    .child(div().text_xs().child(label))
                                    .child(div().flex_1())
                                    .child(
                                        Icon::new(IconName::Eye)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn console(&self, cx: &App) -> AnyElement {
        scroll("console")
            .p_3()
            .gap_1()
            .font_family(cx.theme().mono_font_family.clone())
            .text_xs()
            .child(
                h_flex()
                    .gap_2()
                    .mb_2()
                    .child(Icon::new(IconName::Terminal).small())
                    .child("Meridian — sample output")
                    .child(div().flex_1())
                    .child(muted("Static fixture", cx)),
            )
            .children(
                [
                    ("14:32:08", "workspace", "Loaded 13 sample files"),
                    ("14:32:08", "renderer ", "Metal · Apple GPU · 2× scale"),
                    ("14:32:08", "layout   ", "Restored workspace arrangement"),
                    ("14:32:09", "assets   ", "25 components registered"),
                    ("14:32:09", "preview  ", "Player / Default · 320 × 400"),
                    (
                        "14:32:09",
                        "ready    ",
                        "UI demonstration · no process attached",
                    ),
                ]
                .into_iter()
                .map(|(time, scope, message)| {
                    h_flex()
                        .gap_3()
                        .child(muted(time, cx))
                        .child(div().text_color(cx.theme().primary).child(scope))
                        .child(div().whitespace_nowrap().child(message))
                }),
            )
            .into_any_element()
    }

    fn variables(&self, cx: &App) -> AnyElement {
        scroll("variables")
            .py_1()
            .child(table_header(&["Name", "Value", "Type"], cx))
            .children(
                [
                    ("▸ workspace", "Workspace { … }", "struct"),
                    ("  title", "\"Meridian\"", "String"),
                    ("  panels", "13", "usize"),
                    ("  scale", "1.0", "f32"),
                    ("  is_visible", "true", "bool"),
                    ("▸ selection", "Some(\"Player\")", "Option"),
                ]
                .into_iter()
                .map(|(name, value, ty)| table_row(&[name, value, ty], cx)),
            )
            .into_any_element()
    }

    fn threads(&self, cx: &App) -> AnyElement {
        scroll("threads")
            .py_1()
            .child(table_header(&["Thread", "State", "Queue"], cx))
            .children(
                [
                    ("▸ 1  main", "Waiting", "com.apple.main"),
                    ("▸ 2  renderer", "Idle", "Metal"),
                    ("▸ 3  executor", "Idle", "background"),
                    ("▸ 4  worker", "Idle", "gpui"),
                ]
                .into_iter()
                .map(|(name, state, queue)| table_row(&[name, state, queue], cx)),
            )
            .into_any_element()
    }

    fn performance(&self, cx: &App) -> AnyElement {
        v_flex()
            .size_full()
            .p_3()
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .child(muted("Frame time", cx))
                    .child(div().font_semibold().child("4.2 ms")),
            )
            .child(
                div().flex_1().min_h_0().child(
                    LineChart::new(vec![
                        4.1, 4.5, 4.2, 5.1, 4.3, 4.0, 4.2, 4.8, 4.3, 4.5, 4.1, 4.2,
                    ])
                    .x(|value| SharedString::from(format!("{value}")))
                    .y(|value| *value)
                    .stroke(cx.theme().primary)
                    .natural()
                    .x_axis(false)
                    .y_axis(false)
                    .grid(false)
                    .y_domain(0., 10.)
                    .appear(false),
                ),
            )
            .child(property("Memory", "128 MB", cx))
            .child(muted("Sample measurements", cx))
            .into_any_element()
    }

    fn problems(&self, cx: &App) -> AnyElement {
        scroll("problems")
            .p_3()
            .gap_3()
            .child(muted("2 sample diagnostics · 0 errors", cx))
            .children(
                [
                    ("Unused import: DockLayout", "main.rs:2:21"),
                    ("Field `canvas` is never read", "workspace/mod.rs:10:5"),
                ]
                .into_iter()
                .map(|(message, location)| {
                    h_flex()
                        .gap_2()
                        .child(
                            Icon::new(IconName::TriangleAlert)
                                .small()
                                .text_color(cx.theme().warning),
                        )
                        .child(message)
                        .child(div().flex_1())
                        .child(muted(location, cx))
                }),
            )
            .into_any_element()
    }

    fn search(&self, cx: &App) -> AnyElement {
        scroll("search")
            .p_3()
            .gap_3()
            .child(
                Input::new(&self.input)
                    .small()
                    .prefix(Icon::new(IconName::Search).small()),
            )
            .child(muted("Sample references", cx))
            .children(
                [
                    ("main.rs", "Workspace::new(window, cx)"),
                    ("workspace/mod.rs", "impl Render for Workspace"),
                    ("workspace/canvas.rs", "workspace.canvas.clone()"),
                ]
                .into_iter()
                .map(|(file, text)| {
                    v_flex()
                        .gap_1()
                        .py_2()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(Icon::new(IconName::FileCode).small())
                                .child(file),
                        )
                        .child(muted(text, cx))
                }),
            )
            .into_any_element()
    }
}

impl EventEmitter<PanelEvent> for DemoPanel {}
impl Focusable for DemoPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for DemoPanel {
    fn panel_name(&self) -> &'static str {
        self.surface.title()
    }
}
impl Panel for DemoPanel {
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .text_xs()
            .child(
                Icon::new(self.surface.icon())
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(self.surface.title())
    }
}
impl Render for DemoPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.surface {
            Surface::Project => self.navigator(cx),
            Surface::Search => self.search(cx),
            Surface::Outline => self.outline(cx),
            Surface::Source | Surface::Theme => self.source(cx),
            Surface::Preview => self.preview(cx),
            Surface::Inspector => self.inspector(cx),
            Surface::Controls => self.controls(cx),
            Surface::Console => self.console(cx),
            Surface::Problems => self.problems(cx),
            Surface::Variables => self.variables(cx),
            Surface::Threads => self.threads(cx),
            Surface::Performance => self.performance(cx),
        };
        div()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_sm()
            .child(content)
    }
}

fn scroll(id: &'static str) -> Stateful<Div> {
    v_flex().id(id).size_full().overflow_y_scroll().min_h_0()
}
fn muted(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}
fn section(title: &'static str, cx: &App) -> Div {
    v_flex().child(
        div()
            .text_xs()
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .mb_2()
            .child(title),
    )
}
fn property(label: &str, value: &str, cx: &App) -> Div {
    h_flex()
        .justify_between()
        .gap_2()
        .text_xs()
        .child(muted(label.to_owned(), cx))
        .child(value.to_owned())
}
fn table_header(labels: &[&str], cx: &App) -> Div {
    h_flex()
        .h_7()
        .px_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .children(labels.iter().map(|label| {
            div()
                .flex_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child((*label).to_owned())
        }))
}
fn table_row(values: &[&str], cx: &App) -> Div {
    h_flex()
        .h_7()
        .px_3()
        .gap_2()
        .text_xs()
        .font_family(cx.theme().mono_font_family.clone())
        .children(values.iter().map(|value| {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child((*value).to_owned())
        }))
}
