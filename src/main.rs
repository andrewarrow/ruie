mod hn;
mod browser;
mod model;
mod theme;
mod workspace;

use gpui_kit::*;
actions!(ruie, [Quit, ToggleTheme, About]);

fn main() {
    browser::disable_keychain_prompts().expect("configure embedded browser keychain access");
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
            let client = reqwest_client::ReqwestClient::user_agent("Ruie-HN-Review/0.1")
                .expect("initialize HTTPS client");
            cx.set_http_client(std::sync::Arc::new(client));
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-shift-t", ToggleTheme, None),
            ]);
            cx.set_menus(vec![Menu {
                name: "HN Review".into(), disabled: false,
                items: vec![MenuItem::action("About HN Review…", About), MenuItem::action("Toggle Appearance", ToggleTheme), MenuItem::separator(), MenuItem::action("Quit", Quit)],
            }]);
            workspace::open(cx);
            cx.activate(true);
        });
}

fn about(window: &mut Window, cx: &mut App) {
    use gpui_kit::component::{WindowExt, v_flex};
    window.open_dialog(cx, |dialog, _, _| {
        dialog.title("HN Review").child(v_flex().gap_3()
            .child("Review the latest Hacker News posts. Mark spam, save posts to upvote, or queue a thoughtful reply.")
            .child("Reviews and comment drafts are saved on this device. Voting and posting happen on Hacker News in your browser.")
            .child("Public data from the official Hacker News API. This is an independent reader."))
    });
}
