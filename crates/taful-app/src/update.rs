//! Auto-update (D18): which published release, if any, should replace the
//! running build, and the user's update preferences.
//!
//! Releases are GitHub Releases of the public repository, published by CI:
//! every push to `main` publishes an alpha (`<version>-alpha.<run>`), and a
//! manual CI run publishes a beta (`<version>-beta.<run>`) or a stable
//! release (`<version>`). Each release carries the installer
//! `taful-setup-<version>.exe` and `taful-setup-<version>.exe.sha256`.
//!
//! This module is pure: it parses the GitHub API answer, picks the update and
//! checks a downloaded file's hash. The desktop app does the HTTP requests,
//! runs the installer and draws the UI.

use std::io::Read;
use std::path::Path;

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::services::keys;
use crate::{App, Result};

/// The GitHub repository releases are published to.
pub const REPOSITORY: &str = "fernandolisboa/taful";

/// The GitHub API endpoint listing the newest releases (newest first).
/// `TAFUL_UPDATE_URL` overrides it (tests, a local mirror).
pub fn releases_url() -> String {
    std::env::var("TAFUL_UPDATE_URL")
        .ok()
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| {
            format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=50")
        })
}

/// The release page listing every version.
pub fn releases_page() -> String {
    format!("https://github.com/{REPOSITORY}/releases")
}

/// How long to wait between automatic checks while the app stays open.
pub const CHECK_EVERY_MS: i64 = 6 * 60 * 60 * 1000;

/// Which pre-releases the user accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Stable releases only.
    Stable,
    /// Stable, beta and release-candidate builds.
    Beta,
    /// Everything, including the alpha built from every change.
    Alpha,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Stable, Channel::Beta, Channel::Alpha];

    pub fn setting(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Alpha => "alpha",
        }
    }

    pub fn from_setting(s: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.setting() == s)
    }

    /// Fluent id of the channel's name.
    pub fn label_key(self) -> &'static str {
        match self {
            Channel::Stable => "update-channel-stable",
            Channel::Beta => "update-channel-beta",
            Channel::Alpha => "update-channel-alpha",
        }
    }

    /// The channel a build belongs to: someone running a beta keeps getting
    /// betas until they pick another channel.
    pub fn of(v: &Version) -> Channel {
        let pre = v.pre.as_str();
        if pre.is_empty() {
            Channel::Stable
        } else if pre.starts_with("beta") || pre.starts_with("rc") {
            Channel::Beta
        } else {
            Channel::Alpha
        }
    }

    /// Whether a release of version `v` is offered on this channel.
    pub fn accepts(self, v: &Version) -> bool {
        match self {
            Channel::Stable => v.pre.is_empty(),
            Channel::Beta => Channel::of(v) != Channel::Alpha,
            Channel::Alpha => true,
        }
    }
}

/// Whether an available update is shown or only acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notify {
    /// A card tells the user about a new version and when it is ready.
    Notify,
    /// No card: automatic steps run quietly. Manual checks still report.
    Silent,
}

impl Notify {
    pub const ALL: [Notify; 2] = [Notify::Notify, Notify::Silent];

    pub fn setting(self) -> &'static str {
        match self {
            Notify::Notify => "notify",
            Notify::Silent => "silent",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Notify::Notify => "update-mode-notify",
            Notify::Silent => "update-mode-silent",
        }
    }
}

/// The user's update preferences (settings, `update.*` keys).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefs {
    /// Check at startup and every [`CHECK_EVERY_MS`] (default on).
    pub auto_check: bool,
    /// The chosen channel; `None` follows the running build's channel.
    pub channel: Option<Channel>,
    /// Download a found update without asking (default on).
    pub auto_download: bool,
    /// Install a downloaded update when the app closes (default off).
    pub auto_install: bool,
    pub notify: Notify,
    /// A version the user chose to skip: not offered again automatically.
    pub skipped: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            auto_check: true,
            channel: None,
            auto_download: true,
            auto_install: false,
            notify: Notify::Notify,
            skipped: None,
        }
    }
}

impl Prefs {
    /// The channel in effect for a build of version `current`.
    pub fn channel_for(&self, current: &Version) -> Channel {
        self.channel.unwrap_or_else(|| Channel::of(current))
    }

    /// Whether the user skipped version `v`.
    pub fn is_skipped(&self, v: &Version) -> bool {
        self.skipped.as_deref() == Some(v.to_string().as_str())
    }

    /// Installing on exit needs the file, so it implies downloading.
    pub fn downloads(&self) -> bool {
        self.auto_download || self.auto_install
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

impl App {
    pub fn update_prefs(&self) -> Prefs {
        let d = Prefs::default();
        let flag = |key: &str, default: bool| match self.string_setting(key).as_deref() {
            Some("on") => true,
            Some("off") => false,
            _ => default,
        };
        Prefs {
            auto_check: flag(keys::UPDATE_AUTO_CHECK, d.auto_check),
            channel: self
                .string_setting(keys::UPDATE_CHANNEL)
                .and_then(|s| Channel::from_setting(&s)),
            auto_download: flag(keys::UPDATE_AUTO_DOWNLOAD, d.auto_download),
            auto_install: flag(keys::UPDATE_AUTO_INSTALL, d.auto_install),
            notify: match self.string_setting(keys::UPDATE_MODE).as_deref() {
                Some("silent") => Notify::Silent,
                _ => Notify::Notify,
            },
            skipped: self
                .string_setting(keys::UPDATE_SKIPPED)
                .filter(|s| !s.is_empty()),
        }
    }

    pub fn set_update_prefs(&self, p: &Prefs) -> Result<()> {
        self.set_string_setting(keys::UPDATE_AUTO_CHECK, on_off(p.auto_check))?;
        self.set_string_setting(keys::UPDATE_CHANNEL, p.channel.map_or("", Channel::setting))?;
        self.set_string_setting(keys::UPDATE_AUTO_DOWNLOAD, on_off(p.auto_download))?;
        self.set_string_setting(keys::UPDATE_AUTO_INSTALL, on_off(p.auto_install))?;
        self.set_string_setting(keys::UPDATE_MODE, p.notify.setting())?;
        self.set_string_setting(keys::UPDATE_SKIPPED, p.skipped.as_deref().unwrap_or(""))
    }
}

/// A downloadable file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// A published release that has an installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The release page (notes).
    pub page: String,
    pub installer: Asset,
    /// `<installer>.sha256`. Without it the download cannot be checked and
    /// the release is not offered.
    pub checksum: Asset,
}

impl Release {
    /// Whether this is newer than `current` and its channel is accepted.
    pub fn offered(&self, current: &Version, channel: Channel) -> bool {
        self.version > *current && channel.accepts(&self.version)
    }
}

/// The installer file name for a version (set by CI and `installer/taful.iss`).
pub fn installer_name(v: &Version) -> String {
    format!("taful-setup-{v}.exe")
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Parses the GitHub "list releases" answer. Drafts, tags that are not a
/// version (`v<semver>`) and releases without an installer and its checksum
/// are left out.
pub fn parse_releases(json: &str) -> Result<Vec<Release>, serde_json::Error> {
    let raw: Vec<GhRelease> = serde_json::from_str(json)?;
    Ok(raw
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.strip_prefix('v')?).ok()?;
            let name = installer_name(&version);
            let find = |n: &str| {
                r.assets.iter().find(|a| a.name == n).map(|a| Asset {
                    name: a.name.clone(),
                    url: a.browser_download_url.clone(),
                    size: a.size,
                })
            };
            Some(Release {
                installer: find(&name)?,
                checksum: find(&format!("{name}.sha256"))?,
                version,
                page: r.html_url,
            })
        })
        .collect())
}

/// The newest release offered to a build of `current` on `channel`, if any.
pub fn pick(releases: &[Release], current: &Version, channel: Channel) -> Option<Release> {
    releases
        .iter()
        .filter(|r| r.offered(current, channel))
        .max_by(|a, b| a.version.cmp(&b.version))
        .cloned()
}

/// The hash in a `sha256sum`-style file (`<hex>  <name>` or just `<hex>`),
/// lowercase. `None` if the file has no 64-digit hex hash first.
pub fn parse_checksum(text: &str) -> Option<String> {
    let hex = text.split_whitespace().next()?.to_ascii_lowercase();
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

/// The SHA-256 of a file, lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The installer's command line for an update started by the app: no
/// questions, wait for this process (`pid`) to exit before replacing files,
/// and optionally start the new version when done. `quiet` hides the
/// progress window too.
pub fn installer_args(pid: u32, relaunch: bool, quiet: bool) -> Vec<String> {
    let mut args = vec![
        if quiet { "/VERYSILENT" } else { "/SILENT" }.to_owned(),
        "/SUPPRESSMSGBOXES".to_owned(),
        "/NORESTART".to_owned(),
        format!("/WAITPID={pid}"),
    ];
    if relaunch {
        args.push("/RELAUNCH=1".to_owned());
    }
    args
}

/// The version of a downloaded installer, from its file name.
pub fn installer_version(file_name: &str) -> Option<Version> {
    let v = file_name
        .strip_prefix("taful-setup-")?
        .strip_suffix(".exe")?;
    Version::parse(v).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn gh(tag: &str, draft: bool, assets: &[&str]) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "html_url": format!("https://example.test/{tag}"),
            "draft": draft,
            "prerelease": tag.contains('-'),
            "assets": assets.iter().map(|n| serde_json::json!({
                "name": n,
                "browser_download_url": format!("https://example.test/dl/{n}"),
                "size": 42,
                "content_type": "application/octet-stream",
            })).collect::<Vec<_>>(),
        })
    }

    fn full(tag: &str) -> serde_json::Value {
        let ver = tag.trim_start_matches('v');
        let exe = format!("taful-setup-{ver}.exe");
        let sum = format!("{exe}.sha256");
        gh(tag, false, &[&exe, &sum])
    }

    #[test]
    fn channels() {
        assert_eq!(Channel::of(&v("0.1.0")), Channel::Stable);
        assert_eq!(Channel::of(&v("0.1.0-beta.3")), Channel::Beta);
        assert_eq!(Channel::of(&v("0.1.0-rc.1")), Channel::Beta);
        assert_eq!(Channel::of(&v("0.1.0-alpha.12")), Channel::Alpha);
        assert_eq!(Channel::of(&v("0.1.0-alpha.0.pr70.5")), Channel::Alpha);

        assert!(Channel::Stable.accepts(&v("1.0.0")));
        assert!(!Channel::Stable.accepts(&v("1.0.0-beta.1")));
        assert!(Channel::Beta.accepts(&v("1.0.0-beta.1")));
        assert!(Channel::Beta.accepts(&v("1.0.0")));
        assert!(!Channel::Beta.accepts(&v("1.0.0-alpha.9")));
        assert!(Channel::Alpha.accepts(&v("1.0.0-alpha.9")));

        for c in Channel::ALL {
            assert_eq!(Channel::from_setting(c.setting()), Some(c));
        }
    }

    #[test]
    fn version_order_matches_the_ci_scheme() {
        // A PR build is older than every alpha of the same version, alphas
        // are older than betas, betas than the stable release.
        let order = [
            "0.1.0-alpha.0.pr70.99",
            "0.1.0-alpha.5",
            "0.1.0-alpha.12",
            "0.1.0-beta.13",
            "0.1.0",
            "0.1.1-alpha.20",
        ];
        for w in order.windows(2) {
            assert!(v(w[0]) < v(w[1]), "{} < {}", w[0], w[1]);
        }
    }

    #[test]
    fn parses_github_releases() {
        let json = serde_json::Value::Array(vec![
            full("v0.2.0-alpha.7"),
            gh(
                "v0.3.0",
                true,
                &["taful-setup-0.3.0.exe", "taful-setup-0.3.0.exe.sha256"],
            ),
            gh("v0.2.1", false, &["taful-setup-0.2.1.exe"]),
            full("nightly"),
            full("v0.1.0"),
        ])
        .to_string();
        let rs = parse_releases(&json).unwrap();
        let versions: Vec<String> = rs.iter().map(|r| r.version.to_string()).collect();
        // Draft, missing checksum and a non-version tag are left out.
        assert_eq!(versions, ["0.2.0-alpha.7", "0.1.0"]);
        let r = &rs[0];
        assert_eq!(r.installer.name, "taful-setup-0.2.0-alpha.7.exe");
        assert_eq!(
            r.installer.url,
            "https://example.test/dl/taful-setup-0.2.0-alpha.7.exe"
        );
        assert_eq!(r.checksum.name, "taful-setup-0.2.0-alpha.7.exe.sha256");
        assert_eq!(r.installer.size, 42);
        assert_eq!(r.page, "https://example.test/v0.2.0-alpha.7");
        assert!(parse_releases("{\"message\": \"rate limited\"}").is_err());
    }

    #[test]
    fn picks_the_newest_offered_release() {
        let json = serde_json::Value::Array(vec![
            full("v0.2.1-alpha.9"),
            full("v0.2.0-beta.8"),
            full("v0.1.0"),
            full("v0.1.0-alpha.3"),
        ])
        .to_string();
        let rs = parse_releases(&json).unwrap();
        let pick_v = |cur: &str, c: Channel| pick(&rs, &v(cur), c).map(|r| r.version.to_string());
        assert_eq!(
            pick_v("0.1.0-alpha.3", Channel::Alpha).as_deref(),
            Some("0.2.1-alpha.9")
        );
        assert_eq!(
            pick_v("0.1.0-alpha.3", Channel::Beta).as_deref(),
            Some("0.2.0-beta.8")
        );
        assert_eq!(
            pick_v("0.1.0-alpha.3", Channel::Stable).as_deref(),
            Some("0.1.0")
        );
        assert_eq!(pick_v("0.1.0", Channel::Stable), None);
        assert_eq!(pick_v("0.2.1-alpha.9", Channel::Alpha), None);
        // Never a downgrade, even when the channel narrows.
        assert_eq!(pick_v("0.2.1-alpha.9", Channel::Stable), None);
    }

    #[test]
    fn checksums() {
        let hex = "a".repeat(64);
        assert_eq!(
            parse_checksum(&format!("{hex}  taful-setup-0.1.0.exe\n")),
            Some(hex.clone())
        );
        assert_eq!(parse_checksum(&hex.to_uppercase()), Some(hex));
        assert_eq!(parse_checksum("abc  file"), None);
        assert_eq!(parse_checksum(""), None);
        assert_eq!(parse_checksum(&"g".repeat(64)), None);

        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.bin");
        std::fs::write(&f, b"abc").unwrap();
        assert_eq!(
            sha256_file(&f).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn installer_command_line() {
        assert_eq!(
            installer_args(42, true, false),
            [
                "/SILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/WAITPID=42",
                "/RELAUNCH=1"
            ]
        );
        assert_eq!(installer_args(7, false, true)[0], "/VERYSILENT");
        assert_eq!(installer_args(7, false, true).len(), 4);
        assert_eq!(
            installer_version("taful-setup-0.1.0-alpha.3.exe"),
            Some(v("0.1.0-alpha.3"))
        );
        assert_eq!(
            installer_version(&installer_name(&v("1.2.3"))),
            Some(v("1.2.3"))
        );
        assert_eq!(installer_version("taful-setup-x.exe"), None);
        assert_eq!(installer_version("other.exe"), None);
    }

    #[test]
    fn prefs_round_trip() {
        let app = App::open_in_memory(Arc::new(|| 0)).unwrap();
        let p = app.update_prefs();
        assert_eq!(p, Prefs::default());
        assert_eq!(p.channel_for(&v("0.1.0-beta.2")), Channel::Beta);
        assert!(p.downloads());

        let q = Prefs {
            auto_check: false,
            channel: Some(Channel::Stable),
            auto_download: false,
            auto_install: true,
            notify: Notify::Silent,
            skipped: Some("0.2.0".into()),
        };
        app.set_update_prefs(&q).unwrap();
        assert_eq!(app.update_prefs(), q);
        assert!(q.is_skipped(&v("0.2.0")));
        assert!(!q.is_skipped(&v("0.2.1")));
        assert!(q.downloads(), "installing on exit implies downloading");
        assert_eq!(q.channel_for(&v("0.1.0-alpha.1")), Channel::Stable);

        app.set_update_prefs(&Prefs::default()).unwrap();
        assert_eq!(app.update_prefs(), Prefs::default());
    }
}
