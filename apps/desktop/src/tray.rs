//! The notification-area (system tray) icon and its right-click menu (D18):
//! open the window, check for updates (or restart into a downloaded update),
//! quit. Windows only; other platforms get no icon.
//!
//! The menu's events arrive on the UI thread inside the window procedure;
//! they are forwarded through a channel to a task of the shell.

use futures::channel::mpsc::UnboundedSender;
use taful_app::i18n::Localizer;

/// What a click on the icon or a menu item asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayCommand {
    Open,
    /// "Check for updates", or "Restart to update" once one is ready.
    Update,
    Quit,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use taful_app::logo::{self, Ground};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    /// The icon's size in pixels (Windows scales it to the tray).
    const ICON_SIZE: u32 = 32;

    /// Whether Windows draws the taskbar light (the "Windows mode" of
    /// Settings › Personalization › Colors). Read once, when the icon is
    /// made; dark when Windows does not say.
    fn taskbar_is_light() -> bool {
        windows_registry::CURRENT_USER
            .open(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
            .and_then(|key| key.get_u32("SystemUsesLightTheme"))
            .is_ok_and(|light| light != 0)
    }

    pub struct Tray {
        _icon: TrayIcon,
        open: MenuItem,
        update: MenuItem,
        quit: MenuItem,
    }

    impl Tray {
        pub fn new(i18n: &Localizer, tx: UnboundedSender<TrayCommand>) -> Option<Tray> {
            let open = MenuItem::with_id("open", i18n.tr("tray-open"), true, None);
            let update = MenuItem::with_id("update", i18n.tr("tray-check-updates"), true, None);
            let quit = MenuItem::with_id("quit", i18n.tr("tray-quit"), true, None);
            let menu = Menu::new();
            menu.append_items(&[&open, &update, &PredefinedMenuItem::separator(), &quit])
                .ok()?;
            let menu_tx = tx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                let cmd = match e.id.as_ref() {
                    "open" => TrayCommand::Open,
                    "update" => TrayCommand::Update,
                    "quit" => TrayCommand::Quit,
                    _ => return,
                };
                let _ = menu_tx.unbounded_send(cmd);
            }));
            TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
                let open = matches!(
                    e,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    }
                );
                if open {
                    let _ = tx.unbounded_send(TrayCommand::Open);
                }
            }));
            let ground = if taskbar_is_light() {
                Ground::Light
            } else {
                Ground::Dark
            };
            // Bound first: the color-literal lint flags any call ending in `rgba(`.
            let from_pixels = Icon::from_rgba;
            let icon = from_pixels(logo::pixels(ICON_SIZE, ground), ICON_SIZE, ICON_SIZE).ok()?;
            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_tooltip(taful_app::PRODUCT_NAME)
                .with_icon(icon)
                .build();
            match tray {
                Ok(icon) => Some(Tray {
                    _icon: icon,
                    open,
                    update,
                    quit,
                }),
                Err(e) => {
                    eprintln!("taful: tray icon failed: {e}");
                    None
                }
            }
        }

        /// Relabels the menu (locale change, an update became ready).
        pub fn set_labels(&self, i18n: &Localizer, ready: Option<&str>) {
            self.open.set_text(i18n.tr("tray-open"));
            self.update.set_text(match ready {
                Some(v) => i18n.tr_args("tray-install-update", &[("version", v.to_owned().into())]),
                None => i18n.tr("tray-check-updates"),
            });
            self.quit.set_text(i18n.tr("tray-quit"));
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    /// No tray icon outside Windows (the desktop app ships for Windows).
    pub struct Tray;

    impl Tray {
        pub fn new(_: &Localizer, _: UnboundedSender<TrayCommand>) -> Option<Tray> {
            None
        }

        pub fn set_labels(&self, _: &Localizer, _: Option<&str>) {}
    }
}

pub use imp::Tray;
