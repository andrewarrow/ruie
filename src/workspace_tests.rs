use super::Workspace;
use crate::{hn::Item, model::{Decision, Filter, Store}};
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, px, size};
use gpui_kit::test::TestWindowExt;

#[gpui_kit::test]
fn go_to_post_opens_the_clicked_hn_discussion(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(|cx| crate::theme::install(cx, true));
    for (width, height) in [(390., 844.), (768., 1024.), (1280., 800.), (1440., 900.)] {
        let (handle, workspace) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(), size: size(px(width), px(height)),
                })),
                ..Default::default()
            }, cx, |window, cx| cx.new(|cx| {
                let mut workspace = Workspace::new(false, window, cx);
                workspace.store.merge((1..=3).map(|id| Item {
                    id, time: id, title: format!("Post {id}"), kind: "story".into(),
                    url: format!("https://example.com/{id}"),
                    ..Default::default()
                }).collect());
                workspace.ensure_selection(window, cx);
                workspace
            })).unwrap()
        });

        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).selected, Some(3));
            // Simulate each retained webview having navigated independently.
            workspace.update(cx, |view, _| {
                view.details_browser.url = Some("https://news.ycombinator.com/item?id=1".into());
                view.article_browser.url = Some("https://example.com/3".into());
            });
            window.right_click(("post", 2u64), cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.within("popup-menu").click(5usize, cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let view = workspace.read(cx);
            assert_eq!(view.selected, Some(2));
            assert!(view.mobile_detail);
            assert!(view.post_menu.is_none());
            assert_eq!(view.discussion_url().as_deref(), Some("https://news.ycombinator.com/item?id=2"));
            assert_eq!(view.details_browser.url, None);
            assert_eq!(view.article_browser.url.as_deref(), Some("https://example.com/3"));
            let details = window.find("details-pane").bounds();
            let article = window.find("article-pane").bounds();
            let reader = window.find("reader").bounds();
            let total = details.size.height + article.size.height;
            assert!((f32::from(details.size.height / total) - 0.5).abs() < 0.01);
            assert_eq!(details.origin, reader.origin);
            assert_eq!(article.bottom(), reader.bottom());
            for control in ["previous", "next", "move-inbox", "retry-details-browser", "fallback-article-browser"] {
                assert!(window.try_find(control).is_none());
            }
            if width >= 860. {
                assert!((f32::from(reader.size.width) / (width - 1.) - 0.6).abs() < 0.01);
            }
            assert_eq!(details.bottom(), article.origin.y);
            assert_eq!(details.origin.x, article.origin.x);
            assert_eq!(details.size.width, article.size.width);
            assert!(details.origin.x >= px(0.));
            assert!(details.origin.y >= px(0.));
            assert!(details.right() <= px(width));
            assert!(article.bottom() <= px(height));
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).article_url().as_deref(), Some("https://example.com/2"));
            if width < 860. {
                window.click("back", cx);
                assert!(!workspace.read(cx).mobile_detail);
                window.right_click(("post", 2u64), cx);
            } else {
                window.right_click(("post", 2u64), cx);
            }
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.within("popup-menu").click(5usize, cx)).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).selected, Some(2));
            assert!(workspace.read(cx).mobile_detail);
            window.remove_window();
        }).unwrap();
    }
}

#[gpui_kit::test]
fn post_context_menu_moves_the_clicked_post_and_supports_undo(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(|cx| crate::theme::install(cx, true));
    for (width, height) in [(390., 844.), (768., 1024.), (1280., 800.), (1440., 900.)] {
        let path = std::env::temp_dir().join(format!("ruie-menu-{}-{width}.json", std::process::id()));
        let (handle, workspace) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Default::default(), size: size(px(width), px(height)),
                })),
                ..Default::default()
            }, cx, |window, cx| cx.new(|cx| {
                let mut workspace = Workspace::new(false, window, cx);
                workspace.path = Some(path.clone());
                workspace.store.merge((1..=3).map(|id| Item {
                    id, time: id, title: format!("Post {id}"), kind: "story".into(),
                    ..Default::default()
                }).collect());
                workspace.store.drafts.insert(3, "Keep this draft".into());
                workspace.ensure_selection(window, cx);
                workspace
            })).unwrap()
        });
        for (index, decision) in [Decision::Upvote, Decision::Comment, Decision::Spam].into_iter().enumerate() {
            cx.update_window(handle, |_, window, cx| {
                // The first row is selected; the menu must act on the second row.
                assert_eq!(workspace.read(cx).selected, Some(3));
                window.right_click(("post", 2u64), cx);
            }).unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                let bounds = window.find("popup-menu").bounds();
                assert!(bounds.origin.x >= px(0.) && bounds.right() <= px(width));
                assert!(bounds.origin.y >= px(0.) && bounds.bottom() <= px(height));
                assert!(workspace.read(cx).post_menu.is_some());
                window.within("popup-menu").click(index + 1, cx);
            }).unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                let view = workspace.read(cx);
                assert_eq!(view.store.decisions.get(&2), Some(&decision));
                assert_eq!(view.store.count(Filter::Inbox), 2);
                assert_eq!(view.selected, Some(3));
                assert!(view.post_menu.is_none());
                assert_eq!(view.store.drafts[&3], "Keep this draft");
                assert_eq!(Store::load(&path).unwrap().decisions.get(&2), Some(&decision));
                workspace.update(cx, |view, cx| view.undo(window, cx));
                assert!(!workspace.read(cx).store.decisions.contains_key(&2));
                workspace.update(cx, |view, cx| view.select(Some(3), window, cx));
            }).unwrap();
        }

        // Moving the selected row advances selection to the next inbox post.
        cx.update_window(handle, |_, window, cx| window.right_click(("post", 3u64), cx)).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.within("popup-menu").click(1usize, cx)).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).selected, Some(2));
            workspace.update(cx, |view, cx| view.set_filter(Filter::Upvote, window, cx));
            window.right_click(("post", 3u64), cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.within("popup-menu").click(1usize, cx);
            assert_eq!(workspace.read(cx).store.decisions.get(&3), Some(&Decision::Upvote));
            assert!(window.try_find("popup-menu").is_some());
            window.press("escape", cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert!(workspace.read(cx).post_menu.is_none());
            assert!(window.try_find("popup-menu").is_none());
            window.right_click(("post", 3u64), cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.within("popup-menu").click(2usize, cx)).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).store.count(Filter::Upvote), 0);
            assert_eq!(workspace.read(cx).store.count(Filter::Comment), 1);
            workspace.update(cx, |view, cx| view.set_filter(Filter::Comment, window, cx));
            window.right_click(("post", 3u64), cx);
        }).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.within("popup-menu").click(5usize, cx)).unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(workspace.read(cx).store.count(Filter::Inbox), 3);
            assert!(!Store::load(&path).unwrap().decisions.contains_key(&3));
            window.remove_window();
        }).unwrap();
        std::fs::remove_file(&path).unwrap();
    }
}
