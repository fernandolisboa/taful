//! Settings (spec §15.1 item 8): language, theme and table layout (M6),
//! review retention and the FSRS optimizer, backups, the strategy library folder, and the grading
//! thresholds (read-only).

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AnyElement, Context, PathPromptOptions, Window, div, prelude::*, px};
use taful_app::fmt;
use taful_app::i18n::Locale;
use taful_app::layout::Layout;
use taful_app::theme::BUILTIN_THEMES;

use crate::Ui;
use crate::shell::Shell;
use crate::widgets::{button, card, chip, muted, title};

#[derive(Default)]
pub struct SettingsView {
    /// Result of the last backup action.
    pub message: Option<String>,
    /// A file picked for import, waiting for confirmation.
    pub pending_import: Option<PathBuf>,
    /// The FSRS optimizer is running in the background.
    pub optimizing: bool,
    /// Result of the last optimizer action.
    pub optimizer_message: Option<String>,
    /// Bumped when the database is replaced, so a fit of the old review log
    /// is dropped instead of applied.
    pub optimizer_generation: u64,
}

impl Shell {
    fn set_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(e) = self.app.set_locale(locale) {
            eprintln!("taful: saving the locale failed: {e}");
        }
        let theme = self.ui.theme.clone();
        self.set_ui(Rc::new(Ui::new(locale, theme)), window, cx);
    }

    fn set_theme(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.app.set_theme(id) {
            Ok(theme) => {
                let locale = self.ui.i18n.locale();
                self.set_ui(Rc::new(Ui::new(locale, theme)), window, cx);
            }
            Err(e) => eprintln!("taful: saving the theme failed: {e}"),
        }
    }

    /// Hands the active layout to the table screen.
    fn apply_layout(&mut self, r: taful_app::Result<()>, cx: &mut Context<Self>) {
        if let Err(e) = r {
            eprintln!("taful: saving the layout failed: {e}");
        }
        let (_, layout) = self.app.active_layout();
        self.table.update(cx, |t, cx| {
            t.set_layout(layout);
            cx.notify();
        });
    }

    fn pick_library_dir(&mut self, cx: &mut Context<Self>) {
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
                    let r = s.app.set_library_folder(dir.clone());
                    if let Ok(t) = &r {
                        let t = t.clone();
                        s.table
                            .update(cx, |table, cx| table.set_trainers(Ok(t), cx));
                    }
                    s.report(r, |_| Some(dir.display().to_string()));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Fits the FSRS parameters on the background executor (spec §13).
    fn optimize(&mut self, cx: &mut Context<Self>) {
        if self.settings.optimizing {
            return;
        }
        let job = match self.app.optimize_job() {
            Ok(job) => job,
            Err(e) => {
                self.settings.optimizer_message = Some(self.failed(&e));
                return;
            }
        };
        self.settings.optimizing = true;
        self.settings.optimizer_message = None;
        let generation = self.settings.optimizer_generation;
        let task = cx.background_executor().spawn(async move { job.run() });
        cx.spawn(async move |this, cx| {
            let fitted = task.await;
            this.update(cx, |s, cx| {
                if s.settings.optimizer_generation != generation {
                    return;
                }
                s.settings.optimizing = false;
                let r = fitted.and_then(|p| s.app.apply_parameters(&p));
                s.settings.optimizer_message = Some(match r {
                    Ok(()) => s.ui.i18n.tr("settings-optimizer-done"),
                    Err(taful_app::Error::NothingToFit) => {
                        s.ui.i18n.tr("settings-optimizer-nothing")
                    }
                    Err(e) => s.failed(&e),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn failed(&self, e: &taful_app::Error) -> String {
        self.ui
            .i18n
            .tr_args("settings-failed", &[("error", e.to_string().into())])
    }

    /// Review count, optimize and reset (spec §13).
    fn optimizer_card(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let loc = ui.i18n.locale();
        let status = match self.app.optimizer_status() {
            Ok(s) => s,
            Err(e) => {
                return card(&ui, &p)
                    .child(title(ui.i18n.tr("settings-optimizer"), &ui))
                    .child(muted(self.failed(&e), &ui, &p));
            }
        };
        let count = |n: u32| fmt::number(f64::from(n), 0, loc);
        let available = status.available();
        let running = self.settings.optimizing;
        let mut c = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-optimizer"), &ui))
            .child(muted(ui.i18n.tr("settings-optimizer-body"), &ui, &p))
            .child(div().child(ui.i18n.tr_args(
                "settings-optimizer-count",
                &[("count", count(status.reviews).into())],
            )))
            .child(muted(
                ui.i18n.tr(if status.custom {
                    "settings-optimizer-custom"
                } else {
                    "settings-optimizer-default"
                }),
                &ui,
                &p,
            ));
        if !available {
            c = c.child(muted(
                ui.i18n.tr_args(
                    "settings-optimizer-locked",
                    &[
                        ("need", count(status.min_reviews).into()),
                        ("left", count(status.min_reviews - status.reviews).into()),
                    ],
                ),
                &ui,
                &p,
            ));
        }
        let run_label = ui.i18n.tr(if running {
            "settings-optimizer-running"
        } else {
            "settings-optimizer-run"
        });
        let mut buttons = div().flex().flex_wrap().gap(px(t.spacing.sm)).child(
            button("opt_run", run_label, available, &ui, &p)
                .when(!available || running, |b| b.opacity(0.4).cursor_default())
                .on_click(cx.listener(move |this, _, _, cx| {
                    if available {
                        this.optimize(cx);
                    }
                })),
        );
        if status.custom {
            buttons = buttons.child(
                button(
                    "opt_reset",
                    ui.i18n.tr("settings-optimizer-reset"),
                    false,
                    &ui,
                    &p,
                )
                .when(running, |b| b.opacity(0.4).cursor_default())
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.settings.optimizing {
                        return;
                    }
                    let r = this.app.reset_parameters();
                    this.settings.optimizer_message = Some(match r {
                        Ok(()) => this.ui.i18n.tr("settings-optimizer-reset-done"),
                        Err(e) => this.failed(&e),
                    });
                    cx.notify();
                })),
            );
        }
        c = c.child(buttons);
        if let Some(m) = &self.settings.optimizer_message {
            c = c.child(muted(m.clone(), &ui, &p));
        }
        c
    }

    /// Theme and layout pickers (spec §15.3, §15.4).
    fn appearance_card(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let theme_chips: Vec<_> = BUILTIN_THEMES
            .iter()
            .enumerate()
            .map(|(i, (id, _))| {
                let id = *id;
                chip(
                    ("theme", i),
                    ui.i18n.tr(&format!("theme-{id}")),
                    t.name == id,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_theme(id, window, cx);
                    cx.notify();
                }))
            })
            .collect();
        let (active, layout) = self.app.active_layout();
        let choices = self.app.layout_choices().unwrap_or_default();
        let user_layouts = choices.iter().filter(|c| !c.builtin).count();
        let layout_chips: Vec<_> = choices
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let label = if c.builtin {
                    ui.i18n.tr(&format!("layout-{}", c.name))
                } else {
                    c.name.clone()
                };
                let key = c.key.clone();
                chip(("layout", i), label, c.key == active, &ui, &p).on_click(cx.listener(
                    move |this, _, _, cx| {
                        let r = this.app.set_layout(&key).map(|_| ());
                        this.apply_layout(r, cx);
                        cx.notify();
                    },
                ))
            })
            .collect();
        let active_is_user = active.starts_with("user:");
        let copy_name = ui.i18n.tr_args(
            "settings-layout-copy-name",
            &[("n", (user_layouts + 1).into())],
        );
        let mut buttons = div()
            .flex()
            .flex_wrap()
            .gap(px(t.spacing.sm))
            .child(
                button(
                    "layout_save",
                    ui.i18n.tr("settings-layout-save"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let r = this.app.save_layout_copy(&copy_name).map(|_| ());
                    this.apply_layout(r, cx);
                    cx.notify();
                })),
            )
            .child(
                button(
                    "layout_reset",
                    ui.i18n.tr("settings-layout-reset"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    let r = this.app.reset_layout();
                    this.apply_layout(r, cx);
                    cx.notify();
                })),
            );
        if active_is_user {
            let key = active.clone();
            buttons = buttons.child(
                button(
                    "layout_delete",
                    ui.i18n.tr("settings-layout-delete"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let r = this.app.delete_layout(&key);
                    this.apply_layout(r, cx);
                    cx.notify();
                })),
            );
        }
        card(&ui, &p)
            .child(title(ui.i18n.tr("settings-appearance"), &ui))
            .child(muted(ui.i18n.tr("settings-theme"), &ui, &p))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(t.spacing.sm))
                    .children(theme_chips),
            )
            .child(muted(ui.i18n.tr("settings-layout"), &ui, &p))
            .child(muted(ui.i18n.tr("settings-layout-body"), &ui, &p))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(t.spacing.sm))
                    .children(layout_chips),
            )
            .child(layout_preview(&layout, &ui, &p))
            .child(buttons)
    }

    /// Where the postflop library is read from, and whether it is there.
    fn library_card(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let (ui, p) = (self.ui.clone(), self.palette);
        let t = &ui.theme;
        let mut c = card(&ui, &p)
            .child(title(ui.i18n.tr("settings-library"), &ui))
            .child(muted(ui.i18n.tr("settings-library-body"), &ui, &p));
        if let Ok(tr) = self.app.trainers() {
            c = c
                .child(div().child(ui.i18n.tr_args(
                    "settings-library-folder",
                    &[("path", tr.library_dir.display().to_string().into())],
                )))
                .child(muted(
                    match tr.post() {
                        Some(pf) => ui
                            .i18n
                            .tr_args("settings-library-ok", &[("count", pf.spots().len().into())]),
                        None => ui.i18n.tr("settings-library-missing"),
                    },
                    &ui,
                    &p,
                ));
        }
        c.child(
            div().flex().gap(px(t.spacing.sm)).child(
                button(
                    "library_dir",
                    ui.i18n.tr("settings-library-change"),
                    false,
                    &ui,
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| this.pick_library_dir(cx))),
            ),
        )
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
                                            // A running fit read the old log.
                                            this.settings.optimizer_generation += 1;
                                            this.settings.optimizing = false;
                                            let r = this.app.import_from(&f);
                                            // Strings may follow the imported locale
                                            // (before the report, which `set_ui` clears).
                                            let loc = this.app.locale();
                                            let theme = this.app.theme();
                                            this.set_ui(Rc::new(Ui::new(loc, theme)), window, cx);
                                            this.apply_layout(Ok(()), cx);
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

        let appearance = self.appearance_card(cx);
        let optimizer = self.optimizer_card(cx);
        let library = self.library_card(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(t.spacing.lg))
            .child(language)
            .child(appearance)
            .child(retention_card)
            .child(optimizer)
            .child(backups)
            .child(library)
            .child(grading)
            .child(muted(ui.i18n.tr("settings-later"), &ui, &p))
            .into_any_element()
    }
}

/// A small map of a layout's panels.
fn layout_preview(layout: &Layout, ui: &Ui, p: &crate::theme::Palette) -> gpui::Div {
    const CELL: f32 = 4.;
    let t = &ui.theme;
    let mut map = div()
        .relative()
        .flex_none()
        .w(px(CELL * f32::from(layout.grid.cols)))
        .h(px(CELL * f32::from(layout.grid.rows)))
        .rounded(px(t.radii.sm))
        .bg(p.background)
        .border_1()
        .border_color(p.border);
    for panel in layout.panels.iter().filter(|q| q.visible) {
        let fill = if panel.id == "table" {
            p.felt
        } else {
            p.surface_raised
        };
        map = map.child(
            div()
                .absolute()
                .left(px(CELL * f32::from(panel.x) + 1.))
                .top(px(CELL * f32::from(panel.y) + 1.))
                .w(px(CELL * f32::from(panel.w) - 2.))
                .h(px(CELL * f32::from(panel.h) - 2.))
                .rounded(px(t.radii.sm))
                .bg(fill),
        );
    }
    map
}
