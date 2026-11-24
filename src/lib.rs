//! M0 spike UI (docs/tech-selection.md "M0 门槛"): a master-password input
//! and a demo login list — the minimum surface gates 1–4 verify on a device.
//! Disposable verification code: the real vault state machine and crypto
//! will live in pandex-core, not here.

use nsh_base::theme;
use nsh_core::element::{AutoFill, input};
use nsh_core::prelude::*;

/// The app's UI set (`nsh.json` `"rust"`): what a TSX entry may import from
/// `pandex/native` (`README.md`, "Adding a TSX entry").
#[native_module("pandex/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// Demo logins shown after unlock; no real vault exists yet.
const DEMO_LOGINS: [(&str, &str); 3] = [
    ("GitHub", "octocat@example.com"),
    ("Proton Mail", "octocat@proton.me"),
    ("Example Bank", "octocat"),
];

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    let password = signal(String::new());
    let unlocked = signal(false);
    let logins = signal(DEMO_LOGINS.to_vec());

    // Spike only: any non-empty password "unlocks". pandex-core will own
    // the real unlock/lock state machine (UI 层不做加密决策).
    let unlock = move || {
        if !password.get().is_empty() {
            unlocked.set(true);
        }
    };
    let lock = move || {
        password.set(String::new());
        unlocked.set(false);
    };

    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            view().flex_1().p_safe_area().px_6().py_8().child(
                when(unlocked, move || {
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
                                        .on_press(lock)
                                        .child(
                                            text("Lock")
                                                .text_size(px(17.0))
                                                .text_color(move || theme().primary_foreground),
                                        ),
                                )
                                // M0 gate 2 probe: NSH dialogs are separate
                                // Dialog windows; check FLAG_SECURE leaks.
                                .child(
                                    view()
                                        .px_5()
                                        .py_3()
                                        .rounded(px(10.0))
                                        .bg(move || theme().primary)
                                        .active(|s| s.opacity(0.6))
                                        .on_press(move || {
                                            let answer = nsh_base::alert_dialog()
                                                .title("M0 dialog probe")
                                                .description("FLAG_SECURE leak check")
                                                .cancel("ok", "OK")
                                                .show();
                                            spawn(async move {
                                                let _ = answer.await;
                                            });
                                        })
                                        .child(
                                            text("Dialog")
                                                .text_size(px(17.0))
                                                .text_color(move || theme().primary_foreground),
                                        ),
                                ),
                        )
                        .child(each(
                            logins,
                            |item| item.0,
                            move |item, _| {
                                view()
                                    .py_3()
                                    .gap_1()
                                    .child(text(move || item.get().0).text_size(px(17.0)))
                                    .child(
                                        text(move || item.get().1)
                                            .text_size(px(14.0))
                                            .text_color(move || theme().muted_foreground),
                                    )
                            },
                        ))
                })
                .otherwise(move || {
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
                                // Gate 1: password masking, no suggestions,
                                // no system autofill on the master password.
                                .masked(true)
                                .auto_capitalize(AutoCapitalize::None)
                                .auto_correct(false)
                                .auto_fill(AutoFill::Off)
                                .return_key_type(ReturnKeyType::Go)
                                .on_submit(move |_: &str| unlock()),
                        )
                        .child(
                            view()
                                .px_5()
                                .py_3()
                                .rounded(px(10.0))
                                .bg(move || theme().primary)
                                .active(|s| s.opacity(0.6))
                                .on_press(unlock)
                                .child(
                                    text("Unlock")
                                        .text_size(px(17.0))
                                        .text_color(move || theme().primary_foreground),
                                ),
                        )
                }),
            ),
        )
}
