//! NSH thin shell: user events go to pandex-core and the UI renders snapshots.

use std::cell::RefCell;
use std::rc::Rc;

use nsh_base::theme;
use nsh_core::element::{AutoFill, input};
use nsh_core::prelude::*;
use pandex_core::{Core, EntrySummary, Snapshot};

/// Temporary demo master password provisioning the in-memory P5 vault.
/// Removed in P6 when the first-run "create vault" flow replaces demo setup.
const DEMO_MASTER_PASSWORD: &str = "correct horse";

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
    let state = Rc::new(AppState {
        core: Rc::new(RefCell::new({
            let mut core = Core::new();
            core.create_vault(DEMO_MASTER_PASSWORD)
                .expect("provision demo vault");
            core
        })),
        snapshot: signal(Core::new().snapshot()),
        entries: signal(Core::new().entries()),
    });
    let password = signal(String::new());

    let refresh: Rc<dyn Fn()> = {
        let state = Rc::clone(&state);
        Rc::new(move || {
            let core = state.core.borrow();
            state.snapshot.set(core.snapshot());
            state.entries.set(core.entries());
        })
    };
    let unlock: Rc<dyn Fn()> = {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            let _ = state.core.borrow_mut().unlock(&password.get());
            refresh();
        })
    };
    let lock: Rc<dyn Fn()> = {
        let state = Rc::clone(&state);
        let refresh = Rc::clone(&refresh);
        Rc::new(move || {
            state.core.borrow_mut().lock();
            password.set(String::new());
            refresh();
        })
    };

    let state_for_condition = Rc::clone(&state);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view().flex_1().p_safe_area().px_6().py_8().child(
                when(move || !state_for_condition.snapshot.get().is_locked, {
                    let state = Rc::clone(&state);
                    let lock = Rc::clone(&lock);
                    move || {
                        let lock = Rc::clone(&lock);
                        let state_for_entries = Rc::clone(&state);
                        view()
                            .flex_1()
                            .gap_4()
                            .child(
                                view()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .child(text("Vault").text_size(px(28.0)))
                                    .child(
                                        view()
                                            .px_5()
                                            .py_3()
                                            .rounded(px(10.0))
                                            .bg(move || theme().primary)
                                            .active(|s| s.opacity(0.6))
                                            .on_press(move || lock())
                                            .child(
                                                text("Lock")
                                                    .text_size(px(17.0))
                                                    .text_color(move || theme().primary_foreground),
                                            ),
                                    ),
                            )
                            .child(each(
                                move || state_for_entries.entries.get(),
                                |item| item.title.clone(),
                                move |item, _| {
                                    view()
                                        .py_3()
                                        .gap_1()
                                        .child(text(move || item.get().title).text_size(px(17.0)))
                                        .child(
                                            text(move || item.get().username)
                                                .text_size(px(14.0))
                                                .text_color(move || theme().muted_foreground),
                                        )
                                },
                            ))
                    }
                })
                .otherwise({
                    let unlock = Rc::clone(&unlock);
                    move || {
                        let unlock_for_submit = Rc::clone(&unlock);
                        let unlock_for_press = Rc::clone(&unlock);
                        view()
                            .flex_1()
                            .justify_center()
                            .gap_4()
                            .child(text("Pandex").text_size(px(28.0)))
                            .child(
                                input()
                                    .test_id("master-password")
                                    .w_full()
                                    .px_4()
                                    .py_3()
                                    .text_size(px(17.0))
                                    .value(move || password.get())
                                    .on_change(move |text: &str| password.set(text.to_owned()))
                                    .placeholder("Master password")
                                    .masked(true)
                                    .auto_capitalize(AutoCapitalize::None)
                                    .auto_correct(false)
                                    .auto_fill(AutoFill::Off)
                                    .return_key_type(ReturnKeyType::Go)
                                    .on_submit(move |_: &str| unlock_for_submit()),
                            )
                            .child(
                                view()
                                    .px_5()
                                    .py_3()
                                    .rounded(px(10.0))
                                    .bg(move || theme().primary)
                                    .active(|s| s.opacity(0.6))
                                    .on_press(move || unlock_for_press())
                                    .child(
                                        text("Unlock")
                                            .text_size(px(17.0))
                                            .text_color(move || theme().primary_foreground),
                                    ),
                            )
                    }
                }),
            ),
        )
}
