//! Auto-update (D18): checks GitHub Releases, downloads the installer,
//! verifies its SHA-256 and runs it. The choice of release and the
//! preferences live in `taful_app::update`; this module does the I/O and the
//! update card shown over every screen.
//!
//! Requests run on the background executor (blocking I/O, like the FSRS
//! optimizer). Downloads go to `%LOCALAPPDATA%\Taful\updates`; installers
//! the running build already has are deleted at startup.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, Window, div, prelude::*, px};
use semver::Version;
use taful_app::update::{self, Channel, Notify, Prefs, Release};

use crate::screen::Screen;
use crate::shell::Shell;
use crate::widgets::{button, card, muted, segment, segmented, title, toggle};

/// The version of this build: `TAFUL_VERSION` set by CI (e.g.
/// `0.1.0-alpha.42`), else the workspace version.
pub fn current_version() -> Version {
    let raw = option_env!("TAFUL_VERSION")
        .filter(|v| !v.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    Version::parse(raw).unwrap_or_else(|_| {
        Version::parse(env!("CARGO_PKG_VERSION")).expect("the workspace version is semver")
    })
}

/// Wait after startup before the first automatic check.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(10);
/// HTTP timeout for the release list and the checksum file.
const SMALL_TIMEOUT: Duration = Duration::from_secs(30);
/// HTTP timeout for the installer download.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Where an update stands.
#[derive(Clone)]
pub enum Status {
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    Downloading {
        release: Release,
        /// Bytes written so far.
        done: Arc<AtomicU64>,
    },
    /// Downloaded and verified.
    Ready {
        release: Release,
        path: PathBuf,
    },
    /// The last check or download failed (message for the user).
    Failed(String),
}

impl Status {
    fn busy(&self) -> bool {
        matches!(self, Status::Checking | Status::Downloading { .. })
    }

    /// The release being offered, downloaded or ready, if any.
    pub fn release(&self) -> Option<&Release> {
        match self {
            Status::Available(r) => Some(r),
            Status::Downloading { release, .. } | Status::Ready { release, .. } => Some(release),
            _ => None,
        }
    }
}

pub struct Updater {
    pub current: Version,
    pub prefs: Prefs,
    pub status: Status,
    /// When the app started (first automatic check) and the last check ran.
    started: Instant,
    last_check: Option<Instant>,
    /// The update card was closed: hidden until the next new version.
    pub dismissed: bool,
    /// The last version offered (a newer one brings a closed card back).
    offered: Option<Version>,
    /// The installer was started; nothing else may start it again.
    launched: bool,
}

impl Updater {
    pub fn new(prefs: Prefs) -> Self {
        let current = current_version();
        // Installers the running build already has (or newer leftovers of a
        // failed install, which a check finds again) are not needed.
        cleanup(&updates_dir(), &current);
        Self {
            current,
            prefs,
            status: Status::Idle,
            started: Instant::now(),
            last_check: None,
            dismissed: false,
            offered: None,
            launched: false,
        }
    }

    pub fn channel(&self) -> Channel {
        self.prefs.channel_for(&self.current)
    }

    /// Whether an automatic check is due now.
    fn check_due(&self) -> bool {
        if !self.prefs.auto_check || self.status.busy() || self.launched {
            return false;
        }
        match self.last_check {
            None => self.started.elapsed() >= FIRST_CHECK_AFTER,
            Some(t) => t.elapsed().as_millis() >= update::CHECK_EVERY_MS as u128,
        }
    }

    /// Download progress in [0, 1], if a download is running.
    pub fn progress(&self) -> Option<f32> {
        match &self.status {
            Status::Downloading { release, done } => {
                let total = release.installer.size.max(1) as f32;
                Some((done.load(Ordering::Relaxed) as f32 / total).min(1.))
            }
            _ => None,
        }
    }
}

/// `%LOCALAPPDATA%\Taful\updates` on Windows (machine-local, not roamed),
/// else `<data dir>/updates`.
pub fn updates_dir() -> PathBuf {
    if cfg!(windows)
        && let Some(d) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty())
    {
        return PathBuf::from(d)
            .join(taful_app::PRODUCT_NAME)
            .join("updates");
    }
    taful_app::default_data_dir().join("updates")
}

/// Deletes partial downloads and installers not newer than `current`.
fn cleanup(dir: &Path, current: &Version) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let stale = name.ends_with(".part")
            || update::installer_version(&name).is_some_and(|v| v <= *current);
        if stale {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(format!("Taful/{}", current_version()))
        .build()
        .into()
}

/// The published releases (blocking).
fn fetch_releases() -> Result<Vec<Release>, String> {
    let mut resp = agent(SMALL_TIMEOUT)
        .get(update::releases_url())
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    update::parse_releases(&body).map_err(|e| e.to_string())
}

/// Downloads and verifies `r`'s installer into `dir` (blocking). A file
/// already there with the right hash is reused.
fn download(r: &Release, dir: &Path, done: &AtomicU64) -> Result<PathBuf, String> {
    let agent = agent(DOWNLOAD_TIMEOUT);
    let sum_text = agent
        .get(&r.checksum.url)
        .call()
        .and_then(|mut resp| resp.body_mut().read_to_string())
        .map_err(|e| e.to_string())?;
    let expected = update::parse_checksum(&sum_text)
        .ok_or_else(|| format!("{}: no SHA-256 in the file", r.checksum.name))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(&r.installer.name);
    if update::sha256_file(&path).is_ok_and(|h| h == expected) {
        done.store(r.installer.size, Ordering::Relaxed);
        return Ok(path);
    }
    let part = dir.join(format!("{}.part", r.installer.name));
    let result = (|| -> Result<(), String> {
        let mut resp = agent
            .get(&r.installer.url)
            .call()
            .map_err(|e| e.to_string())?;
        let mut reader = resp.body_mut().as_reader();
        let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            done.fetch_add(n as u64, Ordering::Relaxed);
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let got = update::sha256_file(&part).map_err(|e| e.to_string())?;
        if got != expected {
            return Err(format!(
                "{}: SHA-256 mismatch (expected {expected}, got {got})",
                r.installer.name
            ));
        }
        std::fs::rename(&part, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| path)
}

/// Starts the installer detached; it waits for this process to exit.
fn launch_installer(path: &Path, relaunch: bool, quiet: bool) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(path);
    cmd.args(update::installer_args(std::process::id(), relaunch, quiet));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Outlives the app, with no console.
        const DETACHED_PROCESS: u32 = 8;
        const CREATE_NEW_PROCESS_GROUP: u32 = 512;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().map(|_| ())
}

impl Shell {
    /// Called by the shell's ticker: starts an automatic check when due.
    pub(crate) fn update_tick(&mut self, cx: &mut Context<Self>) {
        if self.updater.check_due() {
            self.check_updates(false, cx);
        }
        if self.updater.progress().is_some() {
            cx.notify();
        }
    }

    /// Looks for a newer release. `manual`: the user asked (settings, tray),
    /// so a skipped version is offered again.
    pub(crate) fn check_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        let u = &mut self.updater;
        if u.status.busy() || u.launched {
            return;
        }
        // A downloaded update stays ready unless a newer one turns up.
        let ready = match &u.status {
            Status::Ready { .. } => Some(u.status.clone()),
            _ => None,
        };
        u.status = Status::Checking;
        u.last_check = Some(Instant::now());
        let task = cx
            .background_executor()
            .spawn(async move { fetch_releases() });
        cx.spawn(async move |this, cx| {
            let fetched = task.await;
            this.update(cx, |s, cx| {
                s.on_checked(fetched, manual, ready, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn on_checked(
        &mut self,
        fetched: Result<Vec<Release>, String>,
        manual: bool,
        ready: Option<Status>,
        cx: &mut Context<Self>,
    ) {
        let u = &mut self.updater;
        let releases = match fetched {
            Ok(r) => r,
            Err(e) => {
                eprintln!("taful: update check failed: {e}");
                u.status = ready.unwrap_or(Status::Failed(e));
                return;
            }
        };
        let found = update::pick(&releases, &u.current, u.channel())
            .filter(|r| manual || !u.prefs.is_skipped(&r.version));
        u.status = match (found, ready) {
            (Some(r), Some(Status::Ready { release, path })) if r.version <= release.version => {
                Status::Ready { release, path }
            }
            (Some(r), _) => {
                if u.offered.as_ref() != Some(&r.version) {
                    u.offered = Some(r.version.clone());
                    u.dismissed = false;
                }
                Status::Available(r)
            }
            (None, Some(ready)) => ready,
            (None, None) => Status::UpToDate,
        };
        if matches!(self.updater.status, Status::Available(_)) && self.updater.prefs.downloads() {
            self.download_update(cx);
        }
    }

    /// Downloads the offered release in the background.
    pub(crate) fn download_update(&mut self, cx: &mut Context<Self>) {
        let Status::Available(release) = self.updater.status.clone() else {
            return;
        };
        let done = Arc::new(AtomicU64::new(0));
        self.updater.status = Status::Downloading {
            release: release.clone(),
            done: done.clone(),
        };
        let r = release.clone();
        let task = cx
            .background_executor()
            .spawn(async move { download(&r, &updates_dir(), &done) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |s, cx| {
                s.updater.status = match result {
                    Ok(path) => Status::Ready { release, path },
                    Err(e) => {
                        eprintln!("taful: update download failed: {e}");
                        Status::Failed(e)
                    }
                };
                s.sync_tray();
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Runs the downloaded installer and quits; the installer starts the new
    /// version when it is done.
    pub(crate) fn install_update(&mut self, cx: &mut Context<Self>) {
        let Status::Ready { path, .. } = &self.updater.status else {
            return;
        };
        match launch_installer(path, true, false) {
            Ok(()) => {
                self.updater.launched = true;
                cx.quit();
            }
            Err(e) => {
                self.updater.status = Status::Failed(e.to_string());
                cx.notify();
            }
        }
    }

    /// On quit: install a ready update quietly if the user asked for that.
    pub(crate) fn install_on_exit(&mut self) {
        let u = &mut self.updater;
        if u.launched || !u.prefs.auto_install {
            return;
        }
        if let Status::Ready { path, .. } = &u.status {
            match launch_installer(path, false, true) {
                Ok(()) => u.launched = true,
                Err(e) => eprintln!("taful: starting the update failed: {e}"),
            }
        }
    }

    /// Skips the offered version: no card for it until a newer one.
    pub(crate) fn skip_update(&mut self) {
        if let Some(r) = self.updater.status.release() {
            self.updater.prefs.skipped = Some(r.version.to_string());
            self.save_update_prefs();
            self.updater.status = Status::UpToDate;
            self.sync_tray();
        }
    }

    pub(crate) fn save_update_prefs(&mut self) {
        if let Err(e) = self.app.set_update_prefs(&self.updater.prefs) {
            eprintln!("taful: storing the update settings failed: {e}");
        }
    }

    /// Changes the preferences; a new channel checks again.
    pub(crate) fn set_update_prefs(&mut self, prefs: Prefs, cx: &mut Context<Self>) {
        let channel_changed = prefs.channel_for(&self.updater.current) != self.updater.channel();
        let now_downloads = prefs.downloads() && !self.updater.prefs.downloads();
        self.updater.prefs = prefs;
        self.save_update_prefs();
        if channel_changed {
            if !matches!(self.updater.status, Status::Downloading { .. }) {
                self.updater.status = Status::Idle;
            }
            self.check_updates(true, cx);
        } else if now_downloads && matches!(self.updater.status, Status::Available(_)) {
            self.download_update(cx);
        }
        cx.notify();
    }

    /// The text for an update in progress or ready, for the card and the
    /// settings tab.
    pub(crate) fn update_status_text(&self) -> String {
        let i18n = &self.ui.i18n;
        let ver = |r: &Release| r.version.to_string();
        match &self.updater.status {
            Status::Idle => i18n.tr("update-status-idle"),
            Status::Checking => i18n.tr("update-status-checking"),
            Status::UpToDate => i18n.tr("update-status-up-to-date"),
            Status::Available(r) => {
                i18n.tr_args("update-status-available", &[("version", ver(r).into())])
            }
            Status::Downloading { release, .. } => {
                let pct = (self.updater.progress().unwrap_or(0.) * 100.).round() as i64;
                i18n.tr_args(
                    "update-status-downloading",
                    &[("version", ver(release).into()), ("pct", pct.into())],
                )
            }
            Status::Ready { release, .. } => {
                i18n.tr_args("update-status-ready", &[("version", ver(release).into())])
            }
            Status::Failed(e) => {
                i18n.tr_args("update-status-failed", &[("error", e.clone().into())])
            }
        }
    }

    /// The buttons that act on the current status (card and settings).
    pub(crate) fn update_actions(
        &self,
        prefix: &'static str,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let (ui, p) = (self.ui.clone(), self.palette);
        let i18n = &ui.i18n;
        let mut out = Vec::new();
        match &self.updater.status {
            Status::Available(_) => out.push(
                button((prefix, 0usize), i18n.tr("update-download"), true, &ui, &p)
                    .on_click(cx.listener(|this, _, _, cx| this.download_update(cx)))
                    .into_any_element(),
            ),
            Status::Ready { .. } => out.push(
                button(
                    (prefix, 1usize),
                    i18n.tr("update-install-now"),
                    true,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
                .into_any_element(),
            ),
            _ => {}
        }
        if let Some(r) = self.updater.status.release() {
            let page = r.page.clone();
            out.push(
                button((prefix, 2usize), i18n.tr("update-notes"), false, &ui, &p)
                    .on_click(move |_, _, cx| cx.open_url(&page))
                    .into_any_element(),
            );
            if !matches!(self.updater.status, Status::Downloading { .. }) {
                out.push(
                    button((prefix, 3usize), i18n.tr("update-skip"), false, &ui, &p)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.skip_update();
                            cx.notify();
                        }))
                        .into_any_element(),
                );
            }
        }
        out
    }

    /// The floating card over every screen (notify mode): a new version, its
    /// download, or an update ready to install.
    pub(crate) fn update_card(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let u = &self.updater;
        let shown = u.prefs.notify == Notify::Notify
            && !u.dismissed
            && !(self.screen == Screen::Settings
                && self.settings.tab == crate::screens::settings::Tab::Updates)
            && matches!(
                u.status,
                Status::Available(_) | Status::Downloading { .. } | Status::Ready { .. }
            );
        if !shown {
            return None;
        }
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let mut body = div().flex().flex_col().gap(px(t.spacing.sm)).child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(t.spacing.md))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.update_status_text()),
                )
                .child(
                    div()
                        .id("update_card_close")
                        .px(px(t.spacing.xs))
                        .rounded(px(t.radii.sm))
                        .text_color(p.text_muted)
                        .cursor_pointer()
                        .hover(|s| s.text_color(p.text))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.updater.dismissed = true;
                            cx.notify();
                        }))
                        .child("×"),
                ),
        );
        if let Some(frac) = u.progress() {
            body = body.child(crate::widgets::progress(frac, &ui, &p));
        }
        if matches!(u.status, Status::Ready { .. }) && u.prefs.auto_install {
            body = body.child(muted(ui.i18n.tr("update-on-exit"), &ui, &p));
        }
        let actions = self.update_actions("update_card", cx);
        if !actions.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(t.spacing.sm))
                    .children(actions),
            );
        }
        Some(
            div()
                .absolute()
                .right(px(t.spacing.xl))
                .bottom(px(t.spacing.xl))
                .w(px(380.))
                .p(px(t.spacing.lg))
                .rounded(px(t.radii.lg))
                .bg(p.surface_raised)
                .border_1()
                .border_color(p.accent)
                .shadow_lg()
                .child(body)
                .into_any_element(),
        )
    }
}

impl Shell {
    /// Settings › Updates: version and status, automatic steps, channel,
    /// notifications.
    pub(crate) fn updates_tab(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let i18n = &ui.i18n;
        let u = &self.updater;
        let prefs = u.prefs.clone();

        let mut actions = vec![
            button(
                "update_check",
                i18n.tr("update-check-now"),
                matches!(
                    u.status,
                    Status::Idle | Status::UpToDate | Status::Failed(_)
                ),
                &ui,
                &p,
            )
            .when(u.status.busy(), |b| b.opacity(0.4).cursor_default())
            .on_click(cx.listener(|this, _, _, cx| this.check_updates(true, cx)))
            .into_any_element(),
        ];
        actions.extend(self.update_actions("update_tab", cx));
        actions.push(
            button("update_all", i18n.tr("update-all-versions"), false, &ui, &p)
                .on_click(|_, _, cx| cx.open_url(&update::releases_page()))
                .into_any_element(),
        );
        let mut status = card(&ui, &p)
            .child(title(i18n.tr("update-title"), &ui))
            .child(div().child(i18n.tr_args(
                "update-installed",
                &[("version", u.current.to_string().into())],
            )))
            .child(muted(self.update_status_text(), &ui, &p));
        if let Some(frac) = u.progress() {
            status = status.child(crate::widgets::progress(frac, &ui, &p));
        }
        if matches!(u.status, Status::Ready { .. }) && prefs.auto_install {
            status = status.child(muted(i18n.tr("update-on-exit"), &ui, &p));
        }
        let status = status.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(t.spacing.sm))
                .children(actions),
        );

        let flag = |id: &'static str, key: &str, on: bool, set: fn(&mut Prefs, bool)| {
            let prefs = prefs.clone();
            toggle(
                id,
                i18n.tr(key),
                Some(i18n.tr(&format!("{key}-body")).into()),
                on,
                &ui,
                &p,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                let mut next = prefs.clone();
                set(&mut next, !on);
                this.set_update_prefs(next, cx);
            }))
        };
        let automatic = card(&ui, &p)
            .child(title(i18n.tr("update-automatic"), &ui))
            .child(flag(
                "update_auto_check",
                "update-auto-check",
                prefs.auto_check,
                |p, v| p.auto_check = v,
            ))
            .child(flag(
                "update_auto_download",
                "update-auto-download",
                prefs.downloads(),
                |p, v| {
                    p.auto_download = v;
                    // Installing on exit needs the download.
                    if !v {
                        p.auto_install = false;
                    }
                },
            ))
            .child(flag(
                "update_auto_install",
                "update-auto-install",
                prefs.auto_install,
                |p, v| p.auto_install = v,
            ));

        let current_channel = u.channel();
        let channel_prefs = prefs.clone();
        let channel = card(&ui, &p)
            .child(title(i18n.tr("update-channel"), &ui))
            .child(muted(i18n.tr("update-channel-body"), &ui, &p))
            .child(div().flex().child(segmented(&ui, &p).children(
                Channel::ALL.into_iter().enumerate().map(|(k, c)| {
                    let prefs = channel_prefs.clone();
                    segment(
                        ("update_channel", k),
                        i18n.tr(c.label_key()),
                        c == current_channel,
                        &ui,
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut next = prefs.clone();
                        next.channel = Some(c);
                        this.set_update_prefs(next, cx);
                    }))
                }),
            )))
            .child(muted(
                i18n.tr(&format!("{}-body", current_channel.label_key())),
                &ui,
                &p,
            ));

        let mode_prefs = prefs.clone();
        let mode = card(&ui, &p)
            .child(title(i18n.tr("update-mode"), &ui))
            .child(div().flex().child(segmented(&ui, &p).children(
                Notify::ALL.into_iter().enumerate().map(|(k, n)| {
                    let prefs = mode_prefs.clone();
                    segment(
                        ("update_mode", k),
                        i18n.tr(n.label_key()),
                        n == prefs.notify,
                        &ui,
                        &p,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut next = prefs.clone();
                        next.notify = n;
                        this.set_update_prefs(next, cx);
                    }))
                }),
            )))
            .child(muted(
                i18n.tr(&format!("{}-body", prefs.notify.label_key())),
                &ui,
                &p,
            ));

        vec![
            status.into_any_element(),
            automatic.into_any_element(),
            channel.into_any_element(),
            mode.into_any_element(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        // CI sets TAFUL_VERSION; locally it is the workspace version.
        let v = current_version();
        assert!(!v.to_string().is_empty());
    }

    #[test]
    fn cleanup_keeps_only_newer_installers() {
        let dir = std::env::temp_dir().join(format!("taful-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "taful-setup-0.1.0-alpha.1.exe",
            "taful-setup-0.1.0-alpha.9.exe",
            "taful-setup-0.1.0-alpha.9.exe.part",
            "notes.txt",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        cleanup(&dir, &Version::parse("0.1.0-alpha.5").unwrap());
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["notes.txt", "taful-setup-0.1.0-alpha.9.exe"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
