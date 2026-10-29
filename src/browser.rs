//! A retained Wry child view whose bounds follow the GPUI layout.
use std::{cell::{Cell, RefCell}, rc::Rc};

use futures::{StreamExt, channel::mpsc};
use gpui_kit::{component::{ActiveTheme, Sizable, StyledExt, button::{Button, ButtonVariants}, h_flex, v_flex}, *};
use gpui_kit::assets::IconName;
use gpui_kit::prelude::FluentBuilder;

enum PageEvent {
    Load(wry::PageLoadEvent, String),
    Open(String),
}

pub(crate) struct Browser {
    raw: Rc<wry::WebView>,
    label: String,
    url: String,
    loading: bool,
    error: Option<String>,
    visible: Cell<bool>,
    _events: Task<()>,
}

pub(crate) fn is_web_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
    })
}

/// Set the process-wide policy before WebKit can request its WebCrypto master key.
pub(crate) fn disable_keychain_prompts() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "Security", kind = "framework")]
        unsafe extern "C" {
            fn SecKeychainSetUserInteractionAllowed(state: u8) -> i32;
        }

        // WebKit accesses the legacy keychain from a background thread when a site
        // serializes CryptoKeys. Deny requests needing approval instead of prompting.
        // This deprecated API still governs that legacy keychain; it affects only
        // this process and leaves existing keychain items and web storage intact.
        // SAFETY: Boolean is UInt8 and OSStatus is Int32 in the macOS Security ABI.
        let status = unsafe { SecKeychainSetUserInteractionAllowed(0) };
        anyhow::ensure!(status == 0, "Couldn't disable keychain prompts (OSStatus {status})");
    }
    Ok(())
}

impl Browser {
    pub(crate) fn new(url: &str, label: &str, window: &mut Window, cx: &mut App) -> anyhow::Result<Entity<Self>> {
        anyhow::ensure!(is_web_url(url), "Expected an HTTP or HTTPS URL");
        let (sender, mut receiver) = mpsc::channel(16);
        let loads = RefCell::new(sender.clone());
        let popups = RefCell::new(sender);
        let builder = wry::WebViewBuilder::new()
            .with_url(url)
            .with_visible(false)
            .with_focused(false)
            .with_navigation_handler(|url| url == "about:blank" || is_web_url(&url))
            .with_on_page_load_handler(move |event, url| {
                let _ = loads.borrow_mut().try_send(PageEvent::Load(event, url));
            })
            .with_new_window_req_handler(move |url, _| {
                if is_web_url(&url) {
                    let _ = popups.borrow_mut().try_send(PageEvent::Open(url));
                }
                wry::NewWindowResponse::Deny
            });
        #[cfg(target_os = "linux")]
        let raw = {
            linux::ensure_gtk(cx)?;
            builder.build_as_child(&linux::X11Parent::new(window)?)?
        };
        #[cfg(not(target_os = "linux"))]
        let raw = builder.build_as_child(&*window)?;

        Ok(cx.new(|cx: &mut Context<Self>| Self {
            raw: Rc::new(raw),
            label: label.to_owned(),
            url: url.to_owned(),
            loading: true,
            error: None,
            visible: Cell::new(true),
            _events: cx.spawn(async move |this, cx| {
                while let Some(event) = receiver.next().await {
                    if this.update(cx, |this, cx| {
                        match event {
                            PageEvent::Load(event, url) => {
                                this.url = url;
                                this.loading = matches!(event, wry::PageLoadEvent::Started);
                            }
                            PageEvent::Open(url) => this.load_url(&url, cx),
                        }
                        cx.notify();
                    }).is_err() { break; }
                }
            }),
        }))
    }

    pub(crate) fn load_url(&mut self, url: &str, cx: &mut Context<Self>) {
        if !is_web_url(url) { return; }
        self.url = url.to_owned();
        self.loading = true;
        // Keep the previous document out of view until this navigation finishes.
        let _ = self.raw.set_visible(false);
        self.error = self.raw.load_url(url).err().map(|_| "Couldn't load this page. Try Reload or open it in your browser.".into());
        cx.notify();
    }

    pub(crate) fn hide(&self) {
        if self.visible.replace(false) { let _ = self.raw.focus_parent(); }
        let _ = self.raw.set_visible(false);
    }

    pub(crate) fn show(&self) { self.visible.set(true); }

    fn command(&mut self, result: wry::Result<()>, cx: &mut Context<Self>) {
        self.error = result.err().map(|_| "Couldn't navigate this page. Try opening it in your browser.".into());
        cx.notify();
    }
}

impl Drop for Browser {
    fn drop(&mut self) { self.hide(); }
}

impl Render for Browser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity().downgrade();
        v_flex().size_full().min_h_0()
            .child(h_flex().h_11().px_3().gap_1().flex_shrink_0().border_b_1().border_color(cx.theme().border)
                .child(Button::new("web-back").small().ghost().icon(IconName::ArrowLeft)
                    .accessibility_label("Back").tooltip("Back")
                    .on_click(cx.listener(|this, _, _, cx| this.command(this.raw.go_back(), cx))))
                .child(Button::new("web-forward").small().ghost().icon(IconName::ArrowRight)
                    .accessibility_label("Forward").tooltip("Forward")
                    .on_click(cx.listener(|this, _, _, cx| this.command(this.raw.go_forward(), cx))))
                .child(Button::new("web-reload").small().ghost().icon(IconName::RefreshCw)
                    .accessibility_label("Reload page").tooltip("Reload page")
                    .on_click(cx.listener(|this, _, _, cx| this.command(this.raw.reload(), cx))))
                .child(div().text_xs().font_medium().child(self.label.clone()))
                .child(div().flex_1().min_w_0().px_2().truncate().text_xs().text_color(cx.theme().muted_foreground).child(self.url.clone()))
                .when(self.loading, |el| el.child(div().text_xs().text_color(cx.theme().muted_foreground).child("Loading…")))
                .child(Button::new("web-external").small().ghost().icon(IconName::ExternalLink)
                    .accessibility_label("Open page in browser").tooltip("Open page in browser")
                    .on_click(cx.listener(|this, _, _, cx| { if is_web_url(&this.url) { cx.open_url(&this.url); } }))))
            .when_some(self.error.clone(), |el, error| el.child(div().px_4().py_2().text_xs().child(error)))
            .child(div().flex_1().min_h_0().occlude().child(canvas(
                move |bounds, window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        if this.loading || this.error.is_some() {
                            let _ = this.raw.set_visible(false);
                            return;
                        }
                        if !this.visible.get() { return; }
                        let visible = bounds.intersect(&window.content_mask().bounds);
                        if visible.is_empty() { this.hide(); return; }
                        #[cfg(target_os = "linux")]
                        let rect = {
                            use gtk::prelude::WidgetExt;
                            use wry::WebViewExtUnix;
                            let factor = window.scale_factor();
                            let _ = this.raw.zoom(f64::from(factor) / f64::from(this.raw.webview().scale_factor().max(1)));
                            wry::Rect {
                                position: wry::dpi::PhysicalPosition::new(f32::from(visible.origin.x) * factor, f32::from(visible.origin.y) * factor).into(),
                                size: wry::dpi::PhysicalSize::new(f32::from(visible.size.width) * factor, f32::from(visible.size.height) * factor).into(),
                            }
                        };
                        #[cfg(not(target_os = "linux"))]
                        let rect = wry::Rect {
                            position: wry::dpi::LogicalPosition::new(f64::from(visible.origin.x), f64::from(visible.origin.y)).into(),
                            size: wry::dpi::LogicalSize::new(f64::from(visible.size.width), f64::from(visible.size.height)).into(),
                        };
                        if this.raw.set_bounds(rect).and_then(|_| this.raw.set_visible(true)).is_err() && this.error.is_none() {
                            this.hide();
                            this.error = Some("Couldn't display this page. Open it in your browser to continue.".into());
                            cx.notify();
                        }
                    });
                },
                |_, _, _, _| {},
            ).size_full()))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};

    pub(super) struct X11Parent(XlibWindowHandle);

    impl X11Parent {
        pub(super) fn new(window: &Window) -> anyhow::Result<Self> {
            match HasWindowHandle::window_handle(window)?.as_raw() {
                RawWindowHandle::Xlib(handle) => Ok(Self(handle)),
                RawWindowHandle::Xcb(handle) => {
                    let mut parent = XlibWindowHandle::new(handle.window.get().into());
                    parent.visual_id = handle.visual_id.map_or(0, |id| id.get().into());
                    Ok(Self(parent))
                }
                _ => anyhow::bail!("The embedded browser requires X11 or XWayland on Linux"),
            }
        }
    }

    impl HasWindowHandle for X11Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            // The workspace drops its webview before the parent GPUI window is destroyed.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(self.0)) })
        }
    }

    struct GtkLoop;
    impl Global for GtkLoop {}

    pub(super) fn ensure_gtk(cx: &mut App) -> anyhow::Result<()> {
        if cx.has_global::<GtkLoop>() { return Ok(()); }
        if !gtk::is_initialized_main_thread() {
            gtk::gdk::set_allowed_backends("x11");
            gtk::init()?;
        }
        cx.set_global(GtkLoop);
        cx.spawn(async move |cx| loop {
            while gtk::events_pending() { gtk::main_iteration_do(false); }
            cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
        }).detach();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::is_web_url;

    #[test]
    fn only_http_documents_can_navigate() {
        for url in ["https://example.com/article", "http://localhost:3000/", "https://news.ycombinator.com/item?id=1"] {
            assert!(is_web_url(url), "{url}");
        }
        for url in ["", "https://", "javascript:alert(1)", "file:///etc/passwd", "data:text/html,test", "mailto:a@example.com", "about:blank"] {
            assert!(!is_web_url(url), "{url}");
        }
    }
}
