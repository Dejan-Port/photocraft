//! The self-hosted update notice (feature `self-hosted-update`, off by default).
//!
//! For a PhotoCraft web build served from someone's own server: a strip above the canvas tells
//! every visitor that a newer release exists, and the administrator can sign in and start the
//! update. The update itself is done on the server by `craft-updated` (craft-libs'
//! `craft-update` crate), reached through the web server at `SERVICE_PATH` next to this page;
//! it checks the administrator's sign-in on every request. Setup: docs/self-hosted-updates.md.
//! This is separate from the desktop app's update check (#2295): nothing here runs on desktop.

use craft_update::client::Backend;
use craft_update::ui::{Strings, UpdateBanner};
use photocraft_ui_egui::i18n;

/// Where the web server proxies `craft-updated`, relative to the page.
#[cfg(target_arch = "wasm32")]
pub const SERVICE_PATH: &str = "craft-update/";

pub struct Updates<B: Backend> {
    banner: UpdateBanner<B>,
    strings: Strings,
    language: Option<&'static str>,
}

impl<B: Backend> Updates<B> {
    pub fn new(backend: B) -> Self {
        Updates { banner: UpdateBanner::new(backend, env!("CARGO_PKG_VERSION")), strings: Strings::english(), language: None }
    }

    /// Draws the notice (when there is one) at the top of `ui`, in the interface language (set
    /// by `PhotocraftApp::logic` each frame); call before the app draws so the canvas sits below it.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let lang = i18n::current();
        if self.language != Some(lang.code()) {
            self.strings = Strings::translated(|english| i18n::tr(lang, english).to_owned());
            self.language = Some(lang.code());
        }
        self.banner.show(ui, &self.strings);
    }

    #[cfg(test)]
    pub fn banner(&mut self) -> &mut UpdateBanner<B> {
        &mut self.banner
    }
}

/// The browser client, waking egui when an answer arrives.
#[cfg(target_arch = "wasm32")]
pub fn browser(ctx: egui::Context) -> Updates<craft_update::web::Client> {
    Updates::new(craft_update::web::Client::new(SERVICE_PATH, move || ctx.request_repaint()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_update::client::{ApiError, Reply};
    use craft_update::protocol::{Job, LoginReply, Status};
    use craft_update::ui::Notice;
    use egui_kittest::kittest::{NodeT, Queryable};
    use photocraft_ui_egui::PhotocraftApp;

    #[derive(Default)]
    struct Fake {
        replies: Vec<Reply>,
        logins: Vec<String>,
        updates: Vec<String>,
        reloads: usize,
    }

    impl Backend for Fake {
        fn request_status(&mut self) {}
        fn request_login(&mut self, username: &str, password: &str) {
            self.logins.push(format!("{username}:{password}"));
        }
        fn request_update(&mut self, token: &str) {
            self.updates.push(token.into());
        }
        fn request_logout(&mut self, _: &str) {}
        fn poll(&mut self) -> Vec<Reply> {
            std::mem::take(&mut self.replies)
        }
        fn reload_page(&mut self) {
            self.reloads += 1;
        }
    }

    struct Shell {
        app: PhotocraftApp,
        updates: Updates<Fake>,
    }

    /// The web shell's order: the app's logic, then the notice, then the app below it.
    impl eframe::App for Shell {
        fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            eframe::App::logic(&mut self.app, ctx, frame);
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            self.updates.show(ui);
            eframe::App::ui(&mut self.app, ui, frame);
        }
    }

    fn shell() -> egui_kittest::Harness<'static, Shell> {
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_eframe(|cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            Shell { app: PhotocraftApp::new(photocraft_engine::Session::new(), Default::default()), updates: Updates::new(Fake::default()) }
        });
        h.run_steps(6);
        h
    }

    fn fake<'h>(h: &'h mut egui_kittest::Harness<'static, Shell>) -> &'h mut Fake {
        h.state_mut().updates.banner().backend_mut()
    }

    fn answer(h: &mut egui_kittest::Harness<'static, Shell>, reply: Reply) {
        fake(h).replies.push(reply);
        h.run_steps(6);
    }

    fn type_into(h: &mut egui_kittest::Harness<'static, Shell>, label: &str, text: &str) {
        h.get_by_label(label).click();
        h.run_steps(1);
        h.get_by_label(label).type_text(text);
        h.run_steps(1);
    }

    fn available() -> Status {
        Status {
            current: env!("CARGO_PKG_VERSION").into(),
            latest: Some("99.0.0".into()),
            release_url: Some("https://github.com/storytold/photocraft/releases/tag/v99.0.0".into()),
            update_available: true,
            checked_at: None,
            job: Job::Idle,
        }
    }

    #[test]
    fn visitors_see_the_notice_above_the_canvas_and_only_the_administrator_can_update() {
        let mut h = shell();
        let menu_before = h.get_by_label("File").rect().top();
        assert!(h.query_by_label_contains("is available").is_none());

        // A visitor: the notice, above the menu bar, with Update now disabled.
        answer(&mut h, Reply::Status(Ok(available())));
        let notice = format!("A new version (99.0.0) is available. This site runs {}.", env!("CARGO_PKG_VERSION"));
        let strip = h.get_by_label(&notice).rect();
        let menu = h.get_by_label("File").rect().top();
        assert!(menu > menu_before && menu >= strip.bottom(), "the menu bar moves below the notice ({menu_before} -> {menu}, strip ends {})", strip.bottom());
        assert!(h.get_by_label("Update now").accesskit_node().is_disabled());
        h.get_by_label("Update now").click();
        h.run_steps(3);
        assert!(fake(&mut h).updates.is_empty());

        // A wrong password keeps it disabled.
        h.get_by_label("Administrator sign-in…").click();
        h.run_steps(4);
        type_into(&mut h, "Username", "admin");
        type_into(&mut h, "Password", "guess");
        h.get_by_label("Sign in").click();
        h.run_steps(3);
        assert_eq!(fake(&mut h).logins, ["admin:guess"]);
        answer(&mut h, Reply::Login(Err(ApiError::Unauthorized)));
        h.get_by_label("Wrong username or password.");
        assert!(h.get_by_label("Update now").accesskit_node().is_disabled());

        // The administrator: enabled, and the click sends the session's token.
        answer(&mut h, Reply::Login(Ok(LoginReply { token: "session".into(), expires_in: 900 })));
        let update = h.get_by_label("Update now");
        assert!(!update.accesskit_node().is_disabled());
        update.click();
        h.run_steps(4);
        assert_eq!(fake(&mut h).updates, ["session"]);
    }

    #[test]
    fn everyone_sees_the_update_run_and_then_a_reload_prompt() {
        let mut h = shell();
        answer(&mut h, Reply::Status(Ok(Status { job: Job::Running { version: "99.0.0".into(), started_at: 1 }, ..available() })));
        h.get_by_label("Updating to 99.0.0… The site keeps working meanwhile.");
        assert!(h.query_by_label("Update now").is_none());
        let done =
            Status { current: "99.0.0".into(), update_available: false, job: Job::Succeeded { version: "99.0.0".into(), finished_at: 2 }, ..available() };
        answer(&mut h, Reply::Status(Ok(done)));
        assert_eq!(h.state_mut().updates.banner().notice(), Some(Notice::Installed { latest: "99.0.0".into() }));
        h.get_by_label("Reload").click();
        h.run_steps(3);
        assert_eq!(fake(&mut h).reloads, 1);
    }
}
