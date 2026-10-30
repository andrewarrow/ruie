use std::path::PathBuf;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, ThemeMode, TitleBar, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    h_flex, v_flex,
};
use gpui_kit::{prelude::FluentBuilder, *};
use crate::{hn::{self, Item}, model::{self, Decision, Filter, Store}, *};

#[cfg(feature = "visual-check")]
#[path = "visual_check.rs"]
pub(crate) mod visual_check;

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;

pub fn open(cx: &mut App) {
    gpui_kit::open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
        window_min_size: Some(size(px(390.), px(560.))),
        ..TitleBar::window_options()
    }, cx, |window, cx| {
        window.set_window_title("HN Review");
        cx.new(|cx| Workspace::new(true, window, cx))
    }).expect("open HN Review");
}

struct Workspace {
    details_browser: BrowserPane,
    article_browser: BrowserPane,
    browser_enabled: bool,
    store: Store,
    path: Option<PathBuf>,
    storage_error: Option<String>,
    filter: Filter,
    selected: Option<u64>,
    search: Entity<InputState>,
    focus: FocusHandle,
    list_scroll: ScrollHandle,
    loading: bool,
    remaining: Vec<u64>,
    error: Option<String>,
    notice: Option<String>,
    history: Vec<(u64, Option<Decision>)>,
    mobile_detail: bool,
    post_menu: Option<WeakEntity<PopupMenu>>,
    _post_menu: Option<Subscription>,
    _fetch: Task<()>,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Default)]
struct BrowserPane {
    browser: Option<Entity<crate::browser::Browser>>,
    url: Option<String>,
    error: Option<String>,
}

impl Workspace {
    fn new(live: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = live.then(model::data_path).flatten();
        let (store, storage_error) = match path.as_ref().map(|path| Store::load(path)) {
            Some(Ok(store)) => (store, None),
            Some(Err(error)) => (Store::default(), Some(error.to_string())),
            None if live => (Store::default(), Some("Application data folder is unavailable; reviews cannot be saved.".into())),
            None => (Store::default(), None),
        };
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search title, site, or author…"));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.ensure_selection(window, cx);
                    this.list_scroll.set_offset(Default::default());
                    cx.notify();
                }
            }),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut this = Self {
            details_browser: BrowserPane::default(), article_browser: BrowserPane::default(), browser_enabled: live,
            store, path, storage_error, filter: Filter::Inbox, selected: None, search, focus,
            list_scroll: ScrollHandle::new(),
            loading: false, remaining: Vec::new(), error: None, notice: None, history: Vec::new(),
            mobile_detail: false,
            post_menu: None, _post_menu: None,
            _fetch: Task::ready(()), _poll: Task::ready(()), _subscriptions: subscriptions,
        };
        this.ensure_selection(window, cx);
        if live {
            this.fetch(false, cx);
            this._poll = cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(std::time::Duration::from_secs(60)).await;
                    if this.update(cx, |this, cx| this.fetch(false, cx)).is_err() { break; }
                }
            });
        }
        this
    }

    fn save(&mut self) {
        if self.storage_error.is_some() { return; }
        if let Some(path) = &self.path {
            if self.store.save(path).is_err() {
                self.storage_error = Some("Could not save reviews. Your changes are in memory; check the application data folder before closing.".into());
            }
        }
    }

    fn visible(&self, cx: &App) -> Vec<u64> {
        self.store.visible(self.filter, &self.search.read(cx).value())
    }

    fn ensure_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        let next = self.selected.filter(|id| visible.contains(id)).or(visible.first().copied());
        if self.selected != next { self.select(next, window, cx); }
    }

    fn select(&mut self, id: Option<u64>, _window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != id {
            for pane in [&self.details_browser, &self.article_browser] {
                if let Some(browser) = &pane.browser { browser.read(cx).hide(); }
            }
        }
        self.selected = id;
        cx.notify();
    }

    fn set_filter(&mut self, filter: Filter, window: &mut Window, cx: &mut Context<Self>) {
        self.filter = filter;
        self.mobile_detail = false;
        self.list_scroll.set_offset(Default::default());
        self.ensure_selection(window, cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn decide(&mut self, decision: Option<Decision>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return; };
        self.decide_post(id, decision, window, cx);
    }

    fn decide_post(&mut self, id: u64, decision: Option<Decision>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.store.posts.contains_key(&id) { return; }
        let old = self.store.decisions.get(&id).copied();
        if old == decision { return; }
        let visible = self.visible(cx);
        let index = visible.iter().position(|post| *post == id).unwrap_or(0);
        self.history.push((id, old));
        if let Some(decision) = decision { self.store.decisions.insert(id, decision); }
        else { self.store.decisions.remove(&id); }
        self.notice = Some(format!("{} · {}", decision.map(Decision::label).unwrap_or("Moved to inbox"), self.store.posts[&id].title()));
        self.save();
        if self.selected == Some(id) {
            let remaining = self.visible(cx);
            let next = remaining.get(index).or_else(|| remaining.last()).copied();
            self.select(next, window, cx);
            if let Some(index) = next.and_then(|id| remaining.iter().position(|post| *post == id)) { self.list_scroll.scroll_to_item(index); }
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((id, previous)) = self.history.pop() {
            if let Some(previous) = previous { self.store.decisions.insert(id, previous); }
            else { self.store.decisions.remove(&id); }
            if !self.filter.matches(previous) { self.filter = Filter::All; }
            self.search.update(cx, |state, cx| state.set_value("", window, cx));
            self.select(Some(id), window, cx);
            if let Some(index) = self.visible(cx).iter().position(|post| *post == id) { self.list_scroll.scroll_to_item(index); }
            self.notice = Some("Last review undone".into());
            self.save();
            cx.notify();
        }
    }

    fn navigate(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible(cx);
        if visible.is_empty() { return; }
        let current = self.selected.and_then(|id| visible.iter().position(|post| *post == id)).unwrap_or(0);
        let index = (current as isize + step).clamp(0, visible.len() as isize - 1) as usize;
        self.select(Some(visible[index]), window, cx);
        self.list_scroll.scroll_to_item(index);
    }

    fn fetch(&mut self, more: bool, cx: &mut Context<Self>) {
        if self.loading { return; }
        self.loading = true;
        self.error = None;
        let client = cx.http_client();
        let remaining = self.remaining.clone();
        self._fetch = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move {
                let ids = if more { remaining } else { hn::get::<Vec<u64>>(&client, "newstories").await? };
                let page: Vec<_> = ids.iter().take(hn::PAGE_SIZE).copied().collect();
                let rest = ids.into_iter().skip(hn::PAGE_SIZE).collect::<Vec<_>>();
                let (posts, failed) = hn::items(client, page).await;
                if posts.is_empty() && failed > 0 { anyhow::bail!("Could not load posts"); }
                Ok::<_, anyhow::Error>((posts, rest, failed))
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok((posts, rest, failed)) => {
                        this.store.merge(posts);
                        this.remaining = rest;
                        this.store.synced_at = Some(hn::now());
                        if failed > 0 { this.error = Some(format!("{failed} posts could not be loaded. Refresh to retry.")); }
                        this.save();
                    }
                    Err(_) => this.error = Some("Couldn't reach Hacker News. Check your connection and try Refresh. Saved posts are still available.".into()),
                }
                // Selection is reconciled in render, where a Window is available.
                cx.notify();
            });
        });
        cx.notify();
    }

    fn sync_browser_pane(pane: &mut BrowserPane, label: &str, url: Option<String>, visible: bool, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !visible || !enabled || url.is_none() {
            if let Some(browser) = &pane.browser { browser.read(cx).hide(); }
            return;
        }
        if let Some(browser) = &pane.browser { browser.read(cx).show(); }
        if pane.url == url { return; }
        let url = url.unwrap();
        pane.url = Some(url.clone());
        pane.error = None;
        if let Some(browser) = &pane.browser {
            browser.update(cx, |browser, cx| browser.load_url(&url, cx));
        } else {
            match crate::browser::Browser::new(&url, label, window, cx) {
                Ok(browser) => pane.browser = Some(browser),
                Err(error) => {
                    eprintln!("Could not create embedded browser: {error:#}");
                    pane.error = Some("Couldn't start the embedded browser. Retry or open the page in your browser.".into());
                }
            }
        }
    }

    fn discussion_url(&self) -> Option<String> {
        self.selected.and_then(|id| self.store.posts.get(&id)).map(Item::discussion_url)
    }

    fn article_url(&self) -> Option<String> {
        self.selected.and_then(|id| self.store.posts.get(&id)).map(|post| post.article_url().unwrap_or_else(|| post.discussion_url()))
    }

    fn toolbar(&self, narrow: bool, cx: &mut Context<Self>) -> AnyElement {
        v_flex().flex_shrink_0().border_b_1().border_color(cx.theme().border)
            .child(h_flex().h_16().px_5().gap_2()
                .child(div().text_lg().font_semibold().child(if self.filter == Filter::Inbox { "Latest posts" } else { self.filter.label() }))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(self.visible(cx).len().to_string()))
                .child(div().flex_1())
                .when(!narrow, |el| el.child(div().mr_2().text_xs().text_color(cx.theme().muted_foreground).child("Newest first")))
                .child(Button::new("refresh").small().ghost().icon(IconName::RefreshCw)
                    .label(if self.loading { "Loading…" } else { "Refresh" }).disabled(self.loading)
                    .tooltip("Refresh newest posts · R")
                    .on_click(cx.listener(|this, _, _, cx| this.fetch(false, cx)))))
            .child(h_flex().px_3().pb_3().gap_1().flex_wrap()
                .children(Filter::ALL.into_iter().map(|filter| {
                    Button::new(("compact-filter", filter as usize)).small().ghost()
                        .when(self.filter == filter, |button| button.bg(cx.theme().list_active))
                        .label(format!("{} {}", filter.label(), self.store.count(filter)))
                        .on_click(cx.listener(move |this, _, window, cx| this.set_filter(filter, window, cx)))
                })))
            .into_any_element()
    }

    fn post_row(&self, post: &Item, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let id = post.id;
        let selected = self.selected == Some(id);
        let workspace = cx.entity().downgrade();
        v_flex().id(("post", id)).w_full().min_w_0().h_12().flex_shrink_0().justify_center().px_4()
            .test_support()
            .relative().border_b_1().border_color(cx.theme().border)
            .when(selected, |el| el.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(cx.theme().primary)))
            .when(selected, |el| el.bg(cx.theme().list_active))
            .hover(|style| style.bg(cx.theme().list_hover))
            .cursor_pointer()
            .child(div().w_full().min_w_0().truncate().text_sm().child(post.title()))
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(hn::age(post.time)))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(Some(id), window, cx);
                this.mobile_detail = true;
                this.list_scroll.scroll_to_item(index);
                window.focus(&this.focus, cx);
            }))
            .context_menu(move |menu, _, cx| {
                let Some(view) = workspace.upgrade() else { return menu; };
                let current = view.read(cx).store.decisions.get(&id).copied();
                let menu_entity = cx.entity();
                view.update(cx, |this, cx| {
                    this.post_menu = Some(menu_entity.downgrade());
                    this._post_menu = Some(cx.subscribe(&menu_entity, |this, _, _: &DismissEvent, cx| {
                        this.post_menu = None;
                        cx.notify();
                    }));
                    cx.notify();
                });
                let mut menu = menu.label("Move to");
                for decision in [Decision::Upvote, Decision::Comment, Decision::Spam] {
                    let workspace = workspace.clone();
                    menu = menu.item(PopupMenuItem::new(decision.label())
                        .checked(current == Some(decision))
                        .disabled(current == Some(decision))
                        .on_click(move |_, window, cx| {
                            let _ = workspace.update(cx, |this, cx| {
                                this.post_menu = None;
                                this.decide_post(id, Some(decision), window, cx);
                            });
                        }));
                }
                if current.is_some() {
                    let workspace = workspace.clone();
                    menu = menu.separator().item(PopupMenuItem::new("Inbox").on_click(move |_, window, cx| {
                        let _ = workspace.update(cx, |this, cx| {
                            this.post_menu = None;
                            this.decide_post(id, None, window, cx);
                        });
                    }));
                }
                let workspace = workspace.clone();
                menu = menu.separator().item(PopupMenuItem::new("Go to post").on_click(move |_, window, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.post_menu = None;
                        this.select(Some(id), window, cx);
                        // A user may have navigated within the webview; force this item's HN page.
                        this.details_browser.url = None;
                        this.details_browser.error = None;
                        this.mobile_detail = true;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    });
                }));
                menu
            })
            .into_any_element()
    }

    fn queue(&self, cx: &mut Context<Self>) -> AnyElement {
        let visible = self.visible(cx);
        v_flex().flex_1().min_w_0().min_h_0().h_full()
            .child(div().px_4().py_3().flex_shrink_0().border_b_1().border_color(cx.theme().border)
                .child(Input::new(&self.search).prefix(Icon::new(IconName::Search).small()).aria_label("Search posts by title, site, or author").cleanable(true)))
            .child(v_flex().id("post-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.list_scroll)
                .when(visible.is_empty(), |el| el.child(v_flex().p_6().gap_2()
                    .child(div().font_medium().child(if self.loading { "Loading latest posts…" } else if !self.search.read(cx).value().is_empty() { "No matching posts" } else if self.filter == Filter::Inbox && !self.store.posts.is_empty() { "You're all caught up" } else if self.filter == Filter::Inbox { "No posts loaded" } else { "No posts in this queue" }))
                    .child(div().text_color(cx.theme().muted_foreground).child(if !self.search.read(cx).value().is_empty() { "Try another title, site, or author." } else if self.filter == Filter::Inbox { "Refresh to check for new posts." } else { "Posts you mark will appear here." }))))
                .children(visible.iter().enumerate().map(|(index, id)| self.post_row(&self.store.posts[id], index, cx))))
            .when(!self.remaining.is_empty(), |el| el.child(div().p_3().flex_shrink_0().border_t_1().border_color(cx.theme().border)
                .child(Button::new("load-more").ghost().w_full().label(if self.loading { "Loading…" } else { "Load older posts" }).disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| this.fetch(true, cx))))))
            .into_any_element()
    }

    fn triage_bar(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let decision = self.selected.and_then(|id| self.store.decisions.get(&id)).copied();
        v_flex().p_4().gap_2().flex_shrink_0().border_t_1().border_color(cx.theme().border)
            .child(h_flex().gap_2().children([
                (Decision::Spam, IconName::Trash, "Spam", "1"),
                (Decision::Upvote, IconName::ArrowUp, "Upvote", "2"),
                (Decision::Comment, IconName::MessageSquare, if compact { "Vote + reply" } else { "Upvote + comment" }, "3"),
            ].into_iter().map(|(value, icon, label, key)| {
                Button::new(("triage", value as usize)).outline().flex_1().h_11().px_2().icon(icon).label(label)
                    .when(decision == Some(value), |button| button.bg(cx.theme().list_active).border_color(cx.theme().primary))
                    .tooltip(format!("Mark {} · {key}", value.label()))
                    .on_click(cx.listener(move |this, _, window, cx| this.decide(Some(value), window, cx)))
            })))
            .when(!compact, |el| el.child(div().text_xs().text_color(cx.theme().muted_foreground).child("1  Spam     2  Upvote     3  Upvote + comment     J / K  Navigate")))
            .into_any_element()
    }

    fn browser_content(&self, details: bool, cx: &mut Context<Self>) -> AnyElement {
        let pane = if details { &self.details_browser } else { &self.article_browser };
        let url = pane.url.clone().or_else(|| if details { self.discussion_url() } else { self.article_url() }).unwrap_or_default();
        if let Some(browser) = &pane.browser {
            browser.clone().into_any_element()
        } else {
            v_flex().flex_1().min_h_0().px_2().py_1().gap_1()
                .child(div().text_xs().text_color(cx.theme().muted_foreground)
                    .child(pane.error.clone().unwrap_or_else(|| "Embedded browser is unavailable in this preview.".into())))
                .child(h_flex().gap_1()
                    .child(Button::new(if details { "retry-details-browser" } else { "retry-article-browser" }).xsmall().outline().label("Retry")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let pane = if details { &mut this.details_browser } else { &mut this.article_browser };
                            pane.url = None;
                            pane.error = None;
                            cx.notify();
                        })))
                    .child(Button::new(if details { "fallback-details-browser" } else { "fallback-article-browser" }).xsmall().ghost().label("Open in browser")
                        .on_click(move |_, _, cx| cx.open_url(&url))))
                .into_any_element()
        }
    }

    fn reader(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(post) = self.selected.and_then(|id| self.store.posts.get(&id)) else {
            return v_flex().flex_1().min_w_0().p_6().text_color(cx.theme().muted_foreground).child("Select a post to review.").into_any_element();
        };
        let id = post.id;
        let decision = self.store.decisions.get(&id).copied();
        v_flex().flex_1().min_w_0().min_h_0().h_full()
            .child(h_flex().h_12().px_5().gap_2().flex_shrink_0().border_b_1().border_color(cx.theme().border)
                .when(compact, |el| el.child(Button::new("back").small().ghost().icon(IconName::ArrowLeft).label("Posts")
                    .on_click(cx.listener(|this, _, _, cx| { this.mobile_detail = false; cx.notify(); }))))
                .child(div().flex_1())
                .when_some(decision, |el, decision| el.child(div().text_xs().text_color(cx.theme().primary).child(decision.label())))
                .when(decision.is_some(), |el| el.child(Button::new("move-inbox").small().ghost().icon(IconName::Inbox).tooltip("Move back to inbox").accessibility_label("Move back to inbox")
                    .on_click(cx.listener(|this, _, window, cx| this.decide(None, window, cx)))))
                .child(Button::new("previous").small().ghost().icon(IconName::ChevronUp).tooltip("Previous post · K").accessibility_label("Previous post")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(-1, window, cx))))
                .child(Button::new("next").small().ghost().icon(IconName::ChevronDown).tooltip("Next post · J").accessibility_label("Next post")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(1, window, cx)))))
            .child(v_flex().flex_1().min_h_0().overflow_hidden().items_stretch()
                .child(div().id("details-pane").test_support().flex_1().flex_grow(3.).flex_shrink_1().flex_basis(px(0.)).min_h_0().min_w_0().overflow_hidden().border_b_1().border_color(cx.theme().border)
                    .child(self.browser_content(true, cx)))
                .child(div().id("article-pane").test_support().flex_1().flex_grow(7.).flex_shrink_1().flex_basis(px(0.)).min_h_0().min_w_0().overflow_hidden()
                    .child(self.browser_content(false, cx))))
            .child(self.triage_bar(compact, cx))
            .into_any_element()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_selection(window, cx);
        let width = window.viewport_size().width;
        let compact = width < px(860.);
        // Native child webviews sit above GPUI overlays, so hide the current page while a menu is open.
        let post_menu_open = self.post_menu.as_ref().and_then(WeakEntity::upgrade).is_some();
        let page_visible = (!compact || self.mobile_detail) && !window.has_active_dialog(cx) && !post_menu_open;
        let discussion_url = self.discussion_url();
        let article_url = self.article_url();
        Self::sync_browser_pane(&mut self.details_browser, "Details", discussion_url, page_visible, self.browser_enabled, window, cx);
        Self::sync_browser_pane(&mut self.article_browser, "Article", article_url, page_visible, self.browser_enabled, window, cx);
        v_flex().id("workspace").track_focus(&self.focus).size_full()
            .bg(cx.theme().background).text_color(cx.theme().foreground).text_sm()
            .on_action(|_: &ToggleTheme, _, cx| crate::theme::install(cx, cx.theme().mode != ThemeMode::Dark))
            .on_action(|_: &About, window, cx| crate::about(window, cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.search.focus_handle(cx).contains_focused(window, cx) { return; }
                let key = event.keystroke.key.as_str();
                if event.keystroke.modifiers.platform && key == "z" { this.undo(window, cx); return; }
                if event.keystroke.modifiers.platform || event.keystroke.modifiers.control || event.keystroke.modifiers.alt { return; }
                match key {
                    "1" => this.decide(Some(Decision::Spam), window, cx),
                    "2" => this.decide(Some(Decision::Upvote), window, cx),
                    "3" => this.decide(Some(Decision::Comment), window, cx),
                    "j" | "down" => this.navigate(1, window, cx),
                    "k" | "up" => this.navigate(-1, window, cx),
                    "r" => this.fetch(false, cx),
                    "/" => this.search.update(cx, |state, cx| state.focus(window, cx)),
                    "escape" => { this.mobile_detail = false; cx.notify(); },
                    _ => {},
                }
            }))
            .child(TitleBar::new().child(h_flex().w_full().pr_3().gap_2()
                .child(div().text_xs().font_medium().child("HN Review"))
                .child(div().flex_1())
                .when(!compact, |el| el.child(div().text_xs().text_color(cx.theme().muted_foreground).child("Hacker News · Newest")))
                .child(Button::new("appearance").xsmall().ghost()
                    .icon(if cx.theme().mode == ThemeMode::Dark { IconName::Sun } else { IconName::Moon })
                    .accessibility_label("Toggle appearance").tooltip("Toggle appearance · ⌘⇧T")
                    .on_click(|_, _, cx| crate::theme::install(cx, cx.theme().mode != ThemeMode::Dark)))))
            .when_some(self.storage_error.clone(), |el, error| el.child(div().px_4().py_2().bg(cx.theme().muted).text_xs().child(error)))
            .when_some(self.error.clone(), |el, error| el.child(h_flex().px_4().py_2().gap_2().bg(cx.theme().muted)
                .child(Icon::new(IconName::WifiOff).small())
                .child(div().flex_1().min_w_0().text_xs().child(error))))
            .child(h_flex().flex_1().min_h_0().min_w_0().items_stretch()
                .child(v_flex().flex_1().min_w_0().min_h_0()
                    .when(!compact || !self.mobile_detail, |el| el.child(self.toolbar(compact, cx)))
                    .child(h_flex().flex_1().min_h_0().min_w_0().items_stretch()
                        .when(!compact || !self.mobile_detail, |el| el.child(div().flex_1().min_w_0().h_full().child(self.queue(cx))))
                        .when(!compact, |el| el.child(div().w(px(1.)).h_full().bg(cx.theme().border)))
                        .when(!compact || self.mobile_detail, |el| el.child(div().flex_1().min_w_0().h_full().child(self.reader(compact, cx)))))))
            .child(h_flex().h_9().px_4().gap_2().flex_shrink_0().border_t_1().border_color(cx.theme().border).bg(cx.theme().sidebar).text_xs().text_color(cx.theme().muted_foreground)
                .child(div().min_w_0().truncate().flex_1().child(self.notice.clone().unwrap_or_else(|| {
                    if self.loading { "Syncing with Hacker News…".into() }
                    else if let Some(time) = self.store.synced_at { format!("Updated {} · {} posts", hn::age(time), self.store.posts.len()) }
                    else { "Hacker News · Newest posts".into() }
                })))
                .when(!self.history.is_empty(), |el| el.child(Button::new("undo").xsmall().ghost().label("Undo").tooltip("Undo last review · ⌘Z")
                    .on_click(cx.listener(|this, _, window, cx| this.undo(window, cx)))))
                .when(!compact, |el| el.child("Refreshes every minute")))
    }
}
