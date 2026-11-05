//! Settings (spec §15.1 item 8): language, review retention, backups,
//! grading thresholds (read-only). Layout, theme and library arrive later.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AnyElement, Context, PathPromptOptions, Window, div, prelude::*, px};
use taful_app::fmt;
use taful_app::i18n::Locale;

use crate::Ui;
use crate::shell::Shell;
use crate::widgets::{button, card, chip, muted, title};

#[derive(Default)]
pub struct SettingsView {
    /// Result of the last backup action.
    pub message: Option<String>,
    /// A file picked for import, waiting for confirmation.
    pub pending_import: Option<PathBuf>,
}

impl Shell {
    fn set_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.app.set_locale(locale) {
            eprintln!("taful: saving the locale failed: {e}");
        }
        self.set_ui(Rc::new(Ui::new(locale)), window, cx);
    }

    fn report<T>(&mut self, r: taful_app::Result<T>, path: impl Fn(&T) -> Option<String>) {
        self.settings.message = Some(match r {
            Ok(v) => self.ui.i18n.tr_args(
                "settings-done",
                &[("path", path(&v).unwrap_or_default().into())],
            ),
            Err(e) => self
                .ui
                .i18n
                .tr_args("settings-failed", &[("error", e.to_string().into())]),
        });
    }

    fn pick_backup_dir(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(dir) = paths.into_iter().next()
            {
                this.update(cx, |s, cx| {
                    let r = s.app.set_backup_dir(&dir);
                    s.report(r, |_| Some(dir.display().to_string()));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        let dir = self
            .app
            .backup_dir()
            .unwrap_or_else(taful_app::default_data_dir);
        let name = taful_app::services::export_file_name(self.app.now());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |s, cx| {
                    let r = s.app.export_to(&path);
                    s.report(r, |_| Some(path.display().to_string()));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn pick_import(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(file) = paths.into_iter().next()
            {
                this.update(cx, |s, cx| {
                    s.settings.pending_import = Some(file);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    pub(crate) fn settings_view(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();

        let language = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-language"), &ui))
            .child(
                div()
                    .flex()
                    .gap(px(t.spacing.sm))
                    .child(
                        chip(
                            "lang_pt",
                            ui.i18n.tr("settings-language-pt"),
                            loc == Locale::PtBr,
                            &ui,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_locale(Locale::PtBr, window, cx);
                            cx.notify();
                        })),
                    )
                    .child(
                        chip(
                            "lang_en",
                            ui.i18n.tr("settings-language-en"),
                            loc == Locale::EnUs,
                            &ui,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_locale(Locale::EnUs, window, cx);
                            cx.notify();
                        })),
                    ),
            );

        let retention = self.app.retention();
        let step = |delta: f64| {
            move |this: &mut Shell,
                  _: &gpui::ClickEvent,
                  _: &mut Window,
                  cx: &mut Context<Shell>| {
                let r = this.app.retention() + delta;
                if let Err(e) = this.app.set_retention(r) {
                    eprintln!("taful: saving retention failed: {e}");
                }
                cx.notify();
            }
        };
        let retention_card = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-retention"), &ui))
            .child(muted(ui.i18n.tr("settings-retention-body"), &ui, &p))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(t.spacing.md))
                    .child(
                        button("ret_down", "−", false, &ui, &p).on_click(cx.listener(step(-0.01))),
                    )
                    .child(div().text_size(px(t.text_size.lg)).child(ui.i18n.tr_args(
                        "settings-retention-value",
                        &[("pct", fmt::number(retention * 100.0, 0, loc).into())],
                    )))
                    .child(button("ret_up", "+", false, &ui, &p).on_click(cx.listener(step(0.01)))),
            );

        let mut backups = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-backups"), &ui))
            .child(muted(ui.i18n.tr("settings-backups-body"), &ui, &p))
            .child(match self.app.backup_dir() {
                Some(d) => div().child(ui.i18n.tr_args(
                    "settings-backup-folder",
                    &[("path", d.display().to_string().into())],
                )),
                None => div().child(ui.i18n.tr("settings-backup-none")),
            })
            .child(muted(
                match self.app.last_backup() {
                    Some(ms) => ui.i18n.tr_args(
                        "settings-backup-last",
                        &[("when", fmt::datetime(ms, loc).into())],
                    ),
                    None => ui.i18n.tr("settings-backup-never"),
                },
                &ui,
                &p,
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(t.spacing.sm))
                    .child(
                        button(
                            "backup_now",
                            ui.i18n.tr("settings-backup-now"),
                            true,
                            &ui,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let r = this.app.backup_now();
                            this.report(r, |p| p.as_ref().map(|p| p.display().to_string()));
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "backup_dir",
                            ui.i18n.tr("settings-backup-change"),
                            false,
                            &ui,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.pick_backup_dir(cx))),
                    )
                    .child(
                        button("export", ui.i18n.tr("settings-export"), false, &ui, &p)
                            .on_click(cx.listener(|this, _, _, cx| this.export(cx))),
                    )
                    .child(
                        button("import", ui.i18n.tr("settings-import"), false, &ui, &p)
                            .on_click(cx.listener(|this, _, _, cx| this.pick_import(cx))),
                    ),
            );
        if let Some(file) = &self.settings.pending_import {
            backups = backups.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(t.spacing.sm))
                    .p(px(t.spacing.md))
                    .rounded(px(t.radii.md))
                    .bg(p.surface_raised)
                    .child(ui.i18n.tr("settings-import-confirm"))
                    .child(muted(file.display().to_string(), &ui, &p))
                    .child(
                        div()
                            .flex()
                            .gap(px(t.spacing.sm))
                            .child(
                                button(
                                    "import_yes",
                                    ui.i18n.tr("settings-import-yes"),
                                    true,
                                    &ui,
                                    &p,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        if let Some(f) = this.settings.pending_import.take() {
                                            let r = this.app.import_from(&f);
                                            // Strings may follow the imported locale
                                            // (before the report, which `set_ui` clears).
                                            let loc = this.app.locale();
                                            this.set_ui(Rc::new(Ui::new(loc)), window, cx);
                                            this.report(r, |_| Some(f.display().to_string()));
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                button("import_no", ui.i18n.tr("settings-cancel"), false, &ui, &p)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.pending_import = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        if let Some(m) = &self.settings.message {
            backups = backups.child(muted(m.clone(), &ui, &p));
        }

        let th = self.app.thresholds();
        let list = |v: &[f64; 3]| {
            v.iter()
                .map(|x| fmt::number_trim(*x, 2, loc))
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let grading = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-grading"), &ui))
            .child(muted(
                ui.i18n
                    .tr_args("settings-grading-body", &[("version", th.version.into())]),
                &ui,
                &p,
            ))
            .child(div().child(ui.i18n.tr_args(
                "settings-grading-abs",
                &[("values", list(&th.abs_bb).into())],
            )))
            .child(div().child(ui.i18n.tr_args(
                "settings-grading-rel",
                &[("values", list(&th.rel_pot_pct).into())],
            )));

        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.lg))
            .child(language)
            .child(retention_card)
            .child(backups)
            .child(grading)
            .child(muted(ui.i18n.tr("settings-later"), &ui, &p))
            .into_any_element()
    }
}
