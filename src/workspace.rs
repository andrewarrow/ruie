use std::{collections::HashMap, path::PathBuf};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, ThemeMode, TitleBar, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    menu::{ContextMenuExt, PopupMenuItem},
    text::TextView,
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
    browser: Option<Entity<crate::browser::Browser>>,
    browser_url: Option<String>,
    browser_error: Option<String>,
    browser_enabled: bool,
    show_details: bool,
    store: Store,
    path: Option<PathBuf>,
    storage_error: Option<String>,
    filter: Filter,
    selected: Option<u64>,
    search: Entity<InputState>,
    draft: Entity<TextareaState>,
    focus: FocusHandle,
    list_scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
    loading: bool,
    remaining: Vec<u64>,
    error: Option<String>,
    notice: Option<String>,
    history: Vec<(u64, Option<Decision>)>,
    comments: HashMap<u64, Vec<Item>>,
    comments_loading: bool,
    comments_error: Option<String>,
    mobile_detail: bool,
    _fetch: Task<()>,
    _comments: Task<()>,
    _poll: Task<()>,
    _subscriptions: Vec<Subscription>,
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
        let draft = cx.new(|cx| TextareaState::new(window, cx).rows(5).placeholder("Add something useful to the discussion…"));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.ensure_selection(window, cx);
                    this.list_scroll.set_offset(Default::default());
                    cx.notify();
                }
            }),
            cx.subscribe(&draft, |this, draft, event, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(id) = this.selected {
                        let text = draft.read(cx).value().to_string();
                        if text.trim().is_empty() { this.store.drafts.remove(&id); }
                        else { this.store.drafts.insert(id, text); }
                        this.save();
                        cx.notify();
                    }
                }
            }),
        ];
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut this = Self {
            browser: None, browser_url: None, browser_error: None, browser_enabled: live, show_details: false,
            store, path, storage_error, filter: Filter::Inbox, selected: None, search, draft, focus,
            list_scroll: ScrollHandle::new(), detail_scroll: ScrollHandle::new(),
            loading: false, remaining: Vec::new(), error: None, notice: None, history: Vec::new(),
            comments: HashMap::new(), comments_loading: false, comments_error: None, mobile_detail: false,
            _fetch: Task::ready(()), _comments: Task::ready(()), _poll: Task::ready(()), _subscriptions: subscriptions,
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

    fn select(&mut self, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = id;
        self.show_details = false;
        let text = id.and_then(|id| self.store.drafts.get(&id)).cloned().unwrap_or_default();
        self.draft.update(cx, |state, cx| state.set_value(text, window, cx));
        self.detail_scroll.set_offset(Default::default());
        self.load_comments(cx);
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

    fn load_comments(&mut self, cx: &mut Context<Self>) {
        self._comments = Task::ready(());
        self.comments_loading = false;
        self.comments_error = None;
        let Some(id) = self.selected else { return; };
        if self.comments.contains_key(&id) { return; }
        let ids: Vec<_> = self.store.posts.get(&id).map(|post| post.kids.iter().take(20).copied().collect()).unwrap_or_default();
        if ids.is_empty() { self.comments.insert(id, Vec::new()); return; }
        self.comments_loading = true;
        let client = cx.http_client();
        self._comments = cx.spawn(async move |this, cx| {
            let (mut comments, failed) = cx.background_spawn(async move {
                let (mut items, failed) = hn::items(client, ids.clone()).await;
                items.sort_by_key(|item| ids.iter().position(|id| *id == item.id).unwrap_or(usize::MAX));
                (items, failed)
            }).await;
            comments.retain(|comment| !comment.deleted && !comment.dead && comment.kind == "comment");
            let _ = this.update(cx, |this, cx| {
                if this.selected != Some(id) { return; }
                this.comments_loading = false;
                if failed > 0 { this.comments_error = Some("Some comments couldn't be loaded.".into()); }
                if failed == 0 || !comments.is_empty() { this.comments.insert(id, comments); }
                cx.notify();
            });
        });
    }

    fn filter_icon(filter: Filter) -> IconName {
        match filter { Filter::Inbox => IconName::Inbox, Filter::Upvote => IconName::ArrowUp, Filter::Comment => IconName::MessageSquare, Filter::Spam => IconName::Trash, Filter::All => IconName::List }
    }

    fn sync_browser(&mut self, visible: bool, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.selected.and_then(|id| self.store.posts.get(&id))
            .map(|post| post.article_url().unwrap_or_else(|| post.discussion_url()));
        if !visible || !self.browser_enabled || url.is_none() {
            if let Some(browser) = &self.browser { browser.read(cx).hide(); }
            return;
        }
        if let Some(browser) = &self.browser { browser.read(cx).show(); }
        if self.browser_url == url { return; }
        let url = url.unwrap();
        self.browser_url = Some(url.clone());
        self.browser_error = None;
        if let Some(browser) = &self.browser {
            browser.update(cx, |browser, cx| browser.load_url(&url, cx));
        } else {
            match crate::browser::Browser::new(&url, window, cx) {
                Ok(browser) => self.browser = Some(browser),
                Err(error) => {
                    eprintln!("Could not create article webview: {error:#}");
                    self.browser_error = Some("Couldn't start the embedded browser. Retry or open the page in your browser.".into());
                }
            }
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex().w(px(196.)).flex_shrink_0().h_full().bg(cx.theme().sidebar)
            .border_r_1().border_color(cx.theme().border).px_3().py_5().gap_1()
            .child(h_flex().gap_2().px_2().mb_6()
                .child(div().size_7().flex().items_center().justify_center().rounded_sm().bg(cx.theme().primary).text_color(cx.theme().primary_foreground).font_bold().child("Y"))
                .child(div().font_semibold().text_base().child("HN Review")))
            .children(Filter::ALL.into_iter().map(|filter| {
                let active = self.filter == filter;
                Button::new(("filter", filter as usize)).ghost().w_full().h_10()
                    .when(active, |button| button.bg(cx.theme().list_active).text_color(cx.theme().foreground))
                    .child(h_flex().w_full().gap_2()
                        .child(Icon::new(Self::filter_icon(filter)).small().text_color(if active { cx.theme().primary } else { cx.theme().muted_foreground }))
                        .child(div().flex_1().text_left().child(filter.label()))
                        .child(div().text_xs().text_color(cx.theme().muted_foreground).child(self.store.count(filter).to_string())))
                    .on_click(cx.listener(move |this, _, window, cx| this.set_filter(filter, window, cx)))
            }))
            .child(div().flex_1())
            .child(v_flex().px_2().gap_2().text_xs().text_color(cx.theme().muted_foreground)
                .child("Reviews stay on this device.")
                .child("Vote and reply on Hacker News."))
            .into_any_element()
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
            .when(narrow, |el| el.child(h_flex().px_3().pb_3().gap_1().flex_wrap()
                .children(Filter::ALL.into_iter().map(|filter| {
                    Button::new(("compact-filter", filter as usize)).small().ghost()
                        .when(self.filter == filter, |button| button.bg(cx.theme().list_active))
                        .label(format!("{} {}", filter.label(), self.store.count(filter)))
                        .on_click(cx.listener(move |this, _, window, cx| this.set_filter(filter, window, cx)))
                }))))
            .into_any_element()
    }

    fn post_row(&self, post: &Item, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let id = post.id;
        let selected = self.selected == Some(id);
        let decision = self.store.decisions.get(&id).copied();
        let workspace = cx.entity().downgrade();
        v_flex().id(("post", id)).w_full().min_w_0().px_5().py_4().gap_2()
            .test_support()
            .relative().border_b_1().border_color(cx.theme().border)
            .when(selected, |el| el.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(cx.theme().primary)))
            .when(selected, |el| el.bg(cx.theme().list_active))
            .hover(|style| style.bg(cx.theme().list_hover))
            .cursor_pointer()
            .child(h_flex().gap_2().text_xs().text_color(cx.theme().muted_foreground)
                .child(div().min_w_0().truncate().flex_1().child(post.domain()))
                .when_some(decision, |el, decision| el.child(div().text_color(cx.theme().primary).child(decision.label())))
                .child(hn::age(post.time)))
            .child(div().text_sm().font_semibold().line_height(rems(1.4)).child(post.title()))
            .child(h_flex().gap_3().text_xs().text_color(cx.theme().muted_foreground)
                .child(h_flex().gap_1().child(Icon::new(IconName::ArrowUp).xsmall()).child(post.score.to_string()))
                .child(h_flex().gap_1().child(Icon::new(IconName::MessageSquare).xsmall()).child(post.descendants.to_string()))
                .child(div().min_w_0().truncate().child(format!("by {}", post.by))))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(Some(id), window, cx);
                this.mobile_detail = true;
                this.list_scroll.scroll_to_item(index);
                window.focus(&this.focus, cx);
            }))
            .context_menu(move |menu, _, cx| {
                let Some(view) = workspace.upgrade() else { return menu; };
                let current = view.read(cx).store.decisions.get(&id).copied();
                let mut menu = menu.label("Move to");
                for decision in [Decision::Upvote, Decision::Comment, Decision::Spam] {
                    let workspace = workspace.clone();
                    menu = menu.item(PopupMenuItem::new(decision.label())
                        .checked(current == Some(decision))
                        .disabled(current == Some(decision))
                        .on_click(move |_, window, cx| {
                            let _ = workspace.update(cx, |this, cx| this.decide_post(id, Some(decision), window, cx));
                        }));
                }
                if current.is_some() {
                    let workspace = workspace.clone();
                    menu = menu.separator().item(PopupMenuItem::new("Inbox").on_click(move |_, window, cx| {
                        let _ = workspace.update(cx, |this, cx| this.decide_post(id, None, window, cx));
                    }));
                }
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

    fn reader(&self, compact: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(post) = self.selected.and_then(|id| self.store.posts.get(&id)) else {
            return v_flex().flex_1().min_w_0().p_6().text_color(cx.theme().muted_foreground).child("Select a post to review.").into_any_element();
        };
        let id = post.id;
        let discussion_url = post.discussion_url();
        let article = post.article_url();
        let decision = self.store.decisions.get(&id).copied();
        v_flex().flex_1().min_w_0().min_h_0().h_full()
            .child(h_flex().h_12().px_5().gap_2().flex_shrink_0().border_b_1().border_color(cx.theme().border)
                .when(compact, |el| el.child(Button::new("back").small().ghost().icon(IconName::ArrowLeft).label("Posts")
                    .on_click(cx.listener(|this, _, _, cx| { this.mobile_detail = false; cx.notify(); }))))
                .child(Button::new("show-article").small().ghost().label("Article")
                    .when(!self.show_details, |button| button.bg(cx.theme().list_active))
                    .on_click(cx.listener(|this, _, _, cx| { this.show_details = false; cx.notify(); })))
                .child(Button::new("show-details").small().ghost().label("Details")
                    .when(self.show_details, |button| button.bg(cx.theme().list_active))
                    .on_click(cx.listener(|this, _, _, cx| { this.show_details = true; cx.notify(); })))
                .child(div().flex_1())
                .when_some(decision, |el, decision| el.child(div().text_xs().text_color(cx.theme().primary).child(decision.label())))
                .when(decision.is_some(), |el| el.child(Button::new("move-inbox").small().ghost().icon(IconName::Inbox).tooltip("Move back to inbox").accessibility_label("Move back to inbox")
                    .on_click(cx.listener(|this, _, window, cx| this.decide(None, window, cx)))))
                .child(Button::new("previous").small().ghost().icon(IconName::ChevronUp).tooltip("Previous post · K").accessibility_label("Previous post")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(-1, window, cx))))
                .child(Button::new("next").small().ghost().icon(IconName::ChevronDown).tooltip("Next post · J").accessibility_label("Next post")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(1, window, cx)))))
            .when(!self.show_details, |el| {
                if let Some(browser) = &self.browser {
                    el.child(div().flex_1().min_h_0().child(browser.clone()))
                } else {
                    let url = post.article_url().unwrap_or_else(|| post.discussion_url());
                    el.child(v_flex().flex_1().p_5().gap_3()
                        .child(self.browser_error.clone().unwrap_or_else(|| "Embedded browser is unavailable in this preview.".into()))
                        .child(h_flex().gap_2()
                            .child(Button::new("retry-browser").small().outline().label("Retry")
                                .on_click(cx.listener(|this, _, _, cx| { this.browser_url = None; cx.notify(); })))
                            .child(Button::new("fallback-browser").small().ghost().label("Open in browser")
                                .on_click(move |_, _, cx| cx.open_url(&url)))))
                }
            })
            .when(self.show_details, |el| el.child(v_flex().id("reader-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.detail_scroll).p_6().gap_4()
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(post.domain()))
                .child(div().text_xl().font_semibold().line_height(rems(1.9)).child(post.title()))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!("{} points · by {} · {}", post.score, post.by, hn::age(post.time))))
                .child(h_flex().gap_2().flex_wrap()
                    .when_some(article, |el, url| el.child(Button::new("open-article").small().outline().icon(IconName::ExternalLink).label("Read article").on_click(move |_, _, cx| cx.open_url(&url))))
                    .child(Button::new("open-hn").small().ghost().icon(IconName::ExternalLink).label("Open on HN")
                        .on_click(move |_, _, cx| cx.open_url(&discussion_url))))
                .when(!post.text.is_empty(), |el| el.child(TextView::html(("story-body", id), post.text.clone()).selectable(true)))
                .when(decision == Some(Decision::Comment), |el| {
                    let url = post.discussion_url();
                    el.child(v_flex().gap_2().mt_2()
                        .child(div().font_medium().child("Comment draft"))
                        .child(Textarea::new(&self.draft).h(px(144.)).aria_label("Comment draft saved on this device"))
                        .child(h_flex().gap_2().flex_wrap()
                            .child(Button::new("copy-draft").small().outline().icon(IconName::Copy).label("Copy & open HN")
                                .disabled(self.draft.read(cx).value().trim().is_empty())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(this.draft.read(cx).value().to_string()));
                                    cx.open_url(&url);
                                    this.notice = Some("Draft copied. Paste it into the discussion on Hacker News.".into());
                                    cx.notify();
                                })))
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child("Saved locally"))))
                })
                .child(h_flex().mt_4().gap_2()
                    .child(div().font_semibold().child("Comments"))
                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child(post.descendants.to_string()))
                    .child(div().flex_1())
                    .child(Button::new("refresh-comments").xsmall().ghost().icon(IconName::RefreshCw).disabled(self.comments_loading)
                        .tooltip("Refresh comments").accessibility_label("Refresh comments")
                        .on_click(cx.listener(move |this, _, _, cx| { this.comments.remove(&id); this.load_comments(cx); cx.notify(); }))))
                .when(self.comments_loading, |el| el.child(div().text_color(cx.theme().muted_foreground).child("Loading comments…")))
                .when_some(self.comments_error.clone(), |el, error| el.child(div().text_color(cx.theme().muted_foreground).child(error)))
                .when(!self.comments_loading && self.comments.get(&id).is_some_and(Vec::is_empty), |el| el.child(div().text_color(cx.theme().muted_foreground).child("No comments yet.")))
                .children(self.comments.get(&id).into_iter().flatten().map(|comment| {
                    v_flex().gap_2().py_3().border_t_1().border_color(cx.theme().border)
                        .child(h_flex().gap_2().text_xs()
                            .child(div().font_medium().child(comment.by.clone()))
                            .child(div().text_color(cx.theme().muted_foreground).child(hn::age(comment.time))))
                        .child(TextView::html(("comment", comment.id), comment.text.clone()).selectable(true))
                }))
                .when(post.descendants > 0, |el| {
                    let url = post.discussion_url();
                    el.child(Button::new("all-comments").small().ghost().label("Full discussion on Hacker News ↗")
                        .on_click(move |_, _, cx| cx.open_url(&url)))
                })))
            .child(self.triage_bar(compact, cx))
            .into_any_element()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_selection(window, cx);
        let width = window.viewport_size().width;
        let compact = width < px(860.);
        let sidebar = width >= px(1100.);
        let page_visible = !self.show_details && (!compact || self.mobile_detail) && !window.has_active_dialog(cx);
        self.sync_browser(page_visible, window, cx);
        v_flex().id("workspace").track_focus(&self.focus).size_full()
            .bg(cx.theme().background).text_color(cx.theme().foreground).text_sm()
            .on_action(|_: &ToggleTheme, _, cx| crate::theme::install(cx, cx.theme().mode != ThemeMode::Dark))
            .on_action(|_: &About, window, cx| crate::about(window, cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.search.focus_handle(cx).contains_focused(window, cx) || this.draft.focus_handle(cx).contains_focused(window, cx) { return; }
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
                .when(sidebar, |el| el.child(self.sidebar(cx)))
                .child(v_flex().flex_1().min_w_0().min_h_0()
                    .when(!compact || !self.mobile_detail, |el| el.child(self.toolbar(!sidebar, cx)))
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
