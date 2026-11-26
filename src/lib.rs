//! NSH thin shell: user events go to pandex-core and the UI renders snapshots.
//!
//! Storage path is injected by the host: `data_dir()` is the app-private
//! directory NSH provides (Android `filesDir/nsh/<id>`, Windows `%APPDATA%/<id>`,
//! Linux `~/.local/share/<id>`). Headless tests have no data directory, so they
//! fall back to an in-memory store and each session starts at first run.
//!
//! The look follows Proton Pass: the Proton-purple theme lives in `nsh.json`
//! `"theme"` (light/dark, following the system), cards for entries, letter
//! avatars, rounded inputs and full-width primary actions.

use std::cell::RefCell;
use std::rc::Rc;

use nsh_base::theme;
use nsh_core::element::{AutoFill, input};
use nsh_core::prelude::*;
use pandex_core::{
    Core, EntryDraft, EntrySummary, Error, FileStore, MemoryStore, Snapshot, VaultStore,
};

#[native_module("pandex/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

struct AppState {
    core: Rc<RefCell<Core>>,
    snapshot: Signal<Snapshot>,
    entries: Signal<Vec<EntrySummary>>,
}

/// Root Pandex screen exported to NSH and TSX.
#[component]
pub fn App() -> impl IntoElement {
    apply_app_theme();
    let store: Box<dyn VaultStore> = match data_dir() {
        Some(dir) => Box::new(FileStore::new(dir.join("vault.pandex"))),
        None => Box::new(MemoryStore::new()),
    };
    let state = Rc::new(AppState {
        core: Rc::new(RefCell::new(Core::new(store).expect("load vault"))),
        snapshot: signal(Core::new(Box::new(MemoryStore::new())).unwrap().snapshot()),
        entries: signal(Vec::new()),
    });

    // Unlock / create password (the two screens are mutually exclusive).
    let password = signal(String::new());
    let confirm = signal(String::new());
    let error = signal(String::new());
    // Minimal entry form: hidden / add (editing_id empty) / edit (id set).
    let form_open = signal(false);
    let editing_id = signal(String::new());
    let f_title = signal(String::new());
    let f_username = signal(String::new());
    let f_password = signal(String::new());

    let refresh: Rc<dyn Fn()> = {
        let state = Rc::clone(&state);
        Rc::new(move || {
            let core = state.core.borrow();
            state.snapshot.set(core.snapshot());
            state.entries.set(core.entries());
        })
    };
    // Seed the initial snapshot from the real core (has_vault drives the branch).
    refresh();

    let create: Rc<dyn Fn()> = {
        let core = Rc::clone(&state.core);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            let pw = password.get();
            if pw.is_empty() {
                error.set("Enter a master password.".to_owned());
                return;
            }
            if pw != confirm.get() {
                error.set("Passwords do not match.".to_owned());
                return;
            }
            let result = core.borrow_mut().create_vault(&pw);
            match result {
                Ok(()) => {
                    password.set(String::new());
                    confirm.set(String::new());
                    error.set(String::new());
                    refresh();
                }
                Err(e) => error.set(error_message(e)),
            }
        })
    };
    let unlock: Rc<dyn Fn()> = {
        let core = Rc::clone(&state.core);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            let result = core.borrow_mut().unlock(&password.get());
            match result {
                Ok(()) => {
                    password.set(String::new());
                    error.set(String::new());
                    refresh();
                }
                Err(e) => error.set(error_message(e)),
            }
        })
    };
    let lock: Rc<dyn Fn()> = {
        let core = Rc::clone(&state.core);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            core.borrow_mut().lock();
            password.set(String::new());
            error.set(String::new());
            refresh();
        })
    };
    let open_add: Rc<dyn Fn()> = Rc::new(move || {
        editing_id.set(String::new());
        f_title.set(String::new());
        f_username.set(String::new());
        f_password.set(String::new());
        error.set(String::new());
        form_open.set(true);
    });
    let open_edit: Rc<dyn Fn(String)> = {
        let core = Rc::clone(&state.core);
        Rc::new(move |id: String| {
            let result = core.borrow().entry(&id);
            match result {
                Ok(e) => {
                    editing_id.set(e.id);
                    f_title.set(e.title);
                    f_username.set(e.username);
                    f_password.set(e.password);
                    error.set(String::new());
                    form_open.set(true);
                }
                Err(e) => error.set(error_message(e)),
            }
        })
    };
    let save_form: Rc<dyn Fn()> = {
        let core = Rc::clone(&state.core);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            let title = f_title.get();
            if title.is_empty() {
                error.set("Enter a title.".to_owned());
                return;
            }
            let username = f_username.get();
            let pw = f_password.get();
            let id = editing_id.get();
            let result = if id.is_empty() {
                core.borrow_mut()
                    .add_entry(EntryDraft {
                        title,
                        username,
                        password: pw,
                        urls: Vec::new(),
                        notes: String::new(),
                    })
                    .map(|_| ())
            } else {
                let existing = core.borrow().entry(&id);
                match existing {
                    Ok(mut e) => {
                        e.title = title;
                        e.username = username;
                        e.password = pw;
                        core.borrow_mut().update_entry(e)
                    }
                    Err(e) => Err(e),
                }
            };
            match result {
                Ok(()) => {
                    form_open.set(false);
                    editing_id.set(String::new());
                    f_title.set(String::new());
                    f_username.set(String::new());
                    f_password.set(String::new());
                    error.set(String::new());
                    refresh();
                }
                Err(e) => error.set(error_message(e)),
            }
        })
    };
    let cancel_form: Rc<dyn Fn()> = Rc::new(move || {
        form_open.set(false);
        editing_id.set(String::new());
        f_title.set(String::new());
        f_username.set(String::new());
        f_password.set(String::new());
        error.set(String::new());
    });
    let delete: Rc<dyn Fn(String)> = {
        let core = Rc::clone(&state.core);
        let refresh = Rc::clone(&refresh);
        Rc::new(move |id: String| {
            let result = core.borrow_mut().delete_entry(&id);
            match result {
                Ok(()) => {
                    error.set(String::new());
                    refresh();
                }
                Err(e) => error.set(error_message(e)),
            }
        })
    };

    let state_cond = Rc::clone(&state);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        // Safe-area padding lives on its own layer so the content padding
        // below cannot override it (a fixed `py` would replace the env inset).
        .child(
            view().flex_1().p_safe_area().child(
                view().flex_1().px_6().py_8().child(
                    when(move || state_cond.snapshot.get().has_vault, {
                        let state = Rc::clone(&state);
                        let unlock = Rc::clone(&unlock);
                        let lock = Rc::clone(&lock);
                        let open_add = Rc::clone(&open_add);
                        let open_edit = Rc::clone(&open_edit);
                        let save_form = Rc::clone(&save_form);
                        let cancel_form = Rc::clone(&cancel_form);
                        let delete = Rc::clone(&delete);
                        move || {
                            let state_unlock = Rc::clone(&state);
                            let unlock_screen = {
                                let unlock = Rc::clone(&unlock);
                                move || {
                                    view()
                                        .flex_1()
                                        .justify_center()
                                        .gap_6()
                                        .child(
                                            view()
                                                .items_center()
                                                .gap_3()
                                                .child(brand_badge(64.0))
                                                .child(
                                                    text("Pandex")
                                                        .text_size(px(28.0))
                                                        .font_weight(FontWeight::BOLD),
                                                )
                                                .child(
                                                    text("Unlock your vault")
                                                        .text_size(px(14.0))
                                                        .text_color(move || {
                                                            theme().muted_foreground
                                                        }),
                                                ),
                                        )
                                        .child(field(
                                            "master-password",
                                            "Master password",
                                            password,
                                            true,
                                            Rc::clone(&unlock),
                                        ))
                                        .child(error_banner(error))
                                        .child(button("Unlock", true, true, Rc::clone(&unlock)))
                                }
                            };
                            let vault_screen = {
                                let state = Rc::clone(&state);
                                let lock = Rc::clone(&lock);
                                let open_add = Rc::clone(&open_add);
                                let open_edit = Rc::clone(&open_edit);
                                let save_form = Rc::clone(&save_form);
                                let cancel_form = Rc::clone(&cancel_form);
                                let delete = Rc::clone(&delete);
                                move || {
                                    let entries = state.entries;
                                    let open_edit = Rc::clone(&open_edit);
                                    let delete = Rc::clone(&delete);
                                    let list = move || {
                                        let open_edit = Rc::clone(&open_edit);
                                        let delete = Rc::clone(&delete);
                                        view().flex_1().gap_3().child(each(
                                            move || entries.get(),
                                            |item| item.id.clone(),
                                            move |item, _| {
                                                let id = item.get().id;
                                                let edit_id = id.clone();
                                                let delete_id = id.clone();
                                                let delete_tid = format!("delete-{id}");
                                                let open_edit = Rc::clone(&open_edit);
                                                let delete = Rc::clone(&delete);
                                                view()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap_3()
                                                    .bg(move || theme().card)
                                                    .rounded(px(14.0))
                                                    .px_4()
                                                    .py_3()
                                                    .child(
                                                        view()
                                                            .flex_row()
                                                            .items_center()
                                                            .gap_3()
                                                            .flex_1()
                                                            .active(|s| s.opacity(0.6))
                                                            .on_press(move || {
                                                                open_edit(edit_id.clone())
                                                            })
                                                            .child(letter_avatar(move || {
                                                                item.get().title.clone()
                                                            }))
                                                            .child(
                                                                view()
                                                                    .flex_1()
                                                                    .gap_1()
                                                                    .child(
                                                                        text(move || {
                                                                            item.get().title
                                                                        })
                                                                        .text_size(px(16.0))
                                                                        .font_weight(
                                                                            FontWeight::SEMIBOLD,
                                                                        ),
                                                                    )
                                                                    .child(
                                                                        text(move || {
                                                                            item.get().username
                                                                        })
                                                                        .text_size(px(13.0))
                                                                        .text_color(move || {
                                                                            theme()
                                                                                .muted_foreground
                                                                        }),
                                                                    ),
                                                            ),
                                                    )
                                                    .child(
                                                        view()
                                                            .px_4()
                                                            .py_2()
                                                            .rounded(px(10.0))
                                                            .bg(move || theme().secondary)
                                                            .active(|s| s.opacity(0.6))
                                                            .test_id(&delete_tid)
                                                            .on_press(move || {
                                                                delete(delete_id.clone())
                                                            })
                                                            .child(
                                                                text("Delete")
                                                                    .text_size(px(13.0))
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(move || {
                                                                        theme()
                                                                            .secondary_foreground
                                                                    }),
                                                            ),
                                                    )
                                            },
                                        ))
                                    };
                                    let form = {
                                        let save_form = Rc::clone(&save_form);
                                        let cancel_form = Rc::clone(&cancel_form);
                                        move || {
                                            view()
                                                .flex_1()
                                                .gap_5()
                                                .child(
                                                    text(move || {
                                                        if editing_id.get().is_empty() {
                                                            "Add entry"
                                                        } else {
                                                            "Edit entry"
                                                        }
                                                    })
                                                    .text_size(px(22.0))
                                                    .font_weight(FontWeight::BOLD),
                                                )
                                                .child(
                                                    view()
                                                        .gap_3()
                                                        .child(field(
                                                            "entry-title",
                                                            "Title",
                                                            f_title,
                                                            false,
                                                            Rc::clone(&save_form),
                                                        ))
                                                        .child(field(
                                                            "entry-username",
                                                            "Username",
                                                            f_username,
                                                            false,
                                                            Rc::clone(&save_form),
                                                        ))
                                                        .child(field(
                                                            "entry-password",
                                                            "Password",
                                                            f_password,
                                                            true,
                                                            Rc::clone(&save_form),
                                                        )),
                                                )
                                                .child(error_banner(error))
                                                .child(button(
                                                    "Save",
                                                    true,
                                                    true,
                                                    Rc::clone(&save_form),
                                                ))
                                                .child(view().items_center().child(button(
                                                    "Cancel",
                                                    false,
                                                    false,
                                                    Rc::clone(&cancel_form),
                                                )))
                                        }
                                    };
                                    view()
                                        .flex_1()
                                        .gap_5()
                                        .child(
                                            view()
                                                .flex_row()
                                                .items_center()
                                                .justify_between()
                                                .child(
                                                    view()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_3()
                                                        .child(brand_badge(36.0))
                                                        .child(
                                                            text("Vault")
                                                                .text_size(px(24.0))
                                                                .font_weight(FontWeight::BOLD),
                                                        ),
                                                )
                                                .child(
                                                    view()
                                                        .flex_row()
                                                        .gap_2()
                                                        .child(button(
                                                            "Add",
                                                            true,
                                                            false,
                                                            Rc::clone(&open_add),
                                                        ))
                                                        .child(button(
                                                            "Lock",
                                                            false,
                                                            false,
                                                            Rc::clone(&lock),
                                                        )),
                                                ),
                                        )
                                        .child(when(move || form_open.get(), form).otherwise(list))
                                }
                            };
                            when(move || state_unlock.snapshot.get().is_locked, unlock_screen)
                                .otherwise(vault_screen)
                        }
                    })
                    .otherwise({
                        let create = Rc::clone(&create);
                        move || {
                            view()
                                .flex_1()
                                .justify_center()
                                .gap_6()
                                .child(
                                    view()
                                        .items_center()
                                        .gap_3()
                                        .child(brand_badge(64.0))
                                        .child(
                                            text("Create your vault")
                                                .text_size(px(26.0))
                                                .font_weight(FontWeight::BOLD),
                                        )
                                        .child(
                                            text(
                                                "Set a master password. It never leaves this device.",
                                            )
                                            .text_size(px(14.0))
                                            .text_color(move || theme().muted_foreground),
                                        ),
                                )
                                .child(
                                    view()
                                        .gap_3()
                                        .child(field(
                                            "create-password",
                                            "Master password",
                                            password,
                                            true,
                                            Rc::clone(&create),
                                        ))
                                        .child(field(
                                            "create-confirm",
                                            "Confirm password",
                                            confirm,
                                            true,
                                            Rc::clone(&create),
                                        )),
                                )
                                .child(error_banner(error))
                                .child(button("Create vault", true, true, Rc::clone(&create)))
                        }
                    }),
                ),
            ),
        )
}

/// Apply the Proton theme from `nsh.json` at runtime so every entry point —
/// the packaged Android/Windows runners, the headless test runner and the
/// hand-written desktop dev runner — renders the same palette, regardless of
/// how each builds its root. `nsh.json` is the single source of the theme.
fn apply_app_theme() {
    let theme = serde_json::from_str::<serde_json::Value>(include_str!("../nsh.json"))
        .ok()
        .and_then(|v| v.get("theme").map(|t| t.to_string()))
        .and_then(|json| nsh_base::Theme::from_json(&json).ok());
    if let Some(theme) = theme {
        set_tokens(theme);
    }
}

/// The Proton-style brand mark: a purple rounded square with a white "P".
fn brand_badge(size: f32) -> impl IntoElement {
    view()
        .size(px(size))
        .rounded(px(size * 0.28))
        .bg(move || theme().primary)
        .items_center()
        .justify_center()
        .child(
            text("P")
                .text_size(px(size * 0.5))
                .font_weight(FontWeight::BOLD)
                .text_color(move || theme().primary_foreground),
        )
}

/// A circular purple avatar showing the title's first letter (reactive).
fn letter_avatar(title: impl Fn() -> String + 'static) -> impl IntoElement {
    view()
        .size(px(40.0))
        .rounded_full()
        .bg(move || theme().primary)
        .items_center()
        .justify_center()
        .child(
            text(move || {
                title()
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".to_owned())
            })
            .text_size(px(18.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || theme().primary_foreground),
        )
}

/// A rounded, card-filled text input bound to a signal; `submit` fires on the
/// return key.
fn field(
    tid: &'static str,
    placeholder: &'static str,
    value: Signal<String>,
    masked: bool,
    submit: Rc<dyn Fn()>,
) -> impl IntoElement {
    input()
        .test_id(tid)
        .w_full()
        .px_4()
        .py_3()
        .text_size(px(16.0))
        .bg(move || theme().card)
        .text_color(move || theme().foreground)
        .rounded(px(12.0))
        .border(px(1.0))
        .border_color(move || theme().input)
        .value(move || value.get())
        .on_change(move |text: &str| value.set(text.to_owned()))
        .placeholder(placeholder)
        .masked(masked)
        .auto_capitalize(AutoCapitalize::None)
        .auto_correct(false)
        .auto_fill(AutoFill::Off)
        .return_key_type(ReturnKeyType::Go)
        .on_submit(move |_: &str| submit())
}

/// A pressable action button; `full` stretches it to the container width.
fn button(
    label: &'static str,
    primary: bool,
    full: bool,
    on_press: Rc<dyn Fn()>,
) -> impl IntoElement {
    let v = view()
        .flex_row()
        .items_center()
        .justify_center()
        .px_5()
        .py_3()
        .rounded(px(12.0))
        .bg(move || {
            if primary {
                theme().primary
            } else {
                theme().secondary
            }
        })
        .active(|s| s.opacity(0.6))
        .on_press(move || on_press())
        .child(
            text(label)
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || {
                    if primary {
                        theme().primary_foreground
                    } else {
                        theme().secondary_foreground
                    }
                }),
        );
    if full { v.w_full() } else { v }
}

/// The error line, shown only when an error is set.
fn error_banner(error: Signal<String>) -> impl IntoElement {
    when(
        move || !error.get().is_empty(),
        move || {
            text(move || error.get())
                .text_size(px(14.0))
                .text_color(move || theme().destructive)
        },
    )
}

/// Map a core error to a user-facing message (the thin shell renders, the core
/// decides).
fn error_message(e: Error) -> String {
    match e {
        Error::EmptyPassword => "Enter a master password.".to_owned(),
        Error::InvalidPassword => "Wrong password.".to_owned(),
        Error::VaultExists => "A vault already exists.".to_owned(),
        Error::Corrupt => "The vault file is damaged.".to_owned(),
        Error::Storage => "Could not read or write the vault file.".to_owned(),
        Error::NoVault => "No vault yet.".to_owned(),
        Error::Locked => "The vault is locked.".to_owned(),
        Error::NotFound => "Entry not found.".to_owned(),
        Error::Crypto => "A cryptographic operation failed.".to_owned(),
    }
}
