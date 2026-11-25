#![forbid(unsafe_code)]

const DEMO_PASSWORD: &str = "correct horse";
const DEMO_SUMMARY: &str = "Vault";
const DEMO_ENTRIES: [EntrySummary; 3] = [
    EntrySummary {
        title: "GitHub",
        username: "octocat@example.com",
    },
    EntrySummary {
        title: "Proton Mail",
        username: "octocat@proton.me",
    },
    EntrySummary {
        title: "Example Bank",
        username: "octocat",
    },
];

#[derive(Debug, Eq, PartialEq)]
pub enum UnlockError {
    EmptyPassword,
    InvalidPassword,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub is_locked: bool,
    pub summary: Option<&'static str>,
    pub item_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntrySummary {
    pub title: &'static str,
    pub username: &'static str,
}

pub struct Core {
    is_locked: bool,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    pub fn new() -> Self {
        Self { is_locked: true }
    }

    pub fn unlock(&mut self, password: &str) -> Result<(), UnlockError> {
        if password.is_empty() {
            return Err(UnlockError::EmptyPassword);
        }

        if password != DEMO_PASSWORD {
            return Err(UnlockError::InvalidPassword);
        }

        self.is_locked = false;
        Ok(())
    }

    pub fn lock(&mut self) {
        self.is_locked = true;
    }

    pub fn snapshot(&self) -> Snapshot {
        if self.is_locked {
            Snapshot {
                is_locked: true,
                summary: None,
                item_count: 0,
            }
        } else {
            Snapshot {
                is_locked: false,
                summary: Some(DEMO_SUMMARY),
                item_count: DEMO_ENTRIES.len(),
            }
        }
    }

    pub fn entries(&self) -> Vec<EntrySummary> {
        if self.is_locked {
            Vec::new()
        } else {
            DEMO_ENTRIES.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Core, EntrySummary, Snapshot, UnlockError};

    #[test]
    fn starts_locked() {
        let core = Core::new();

        assert_eq!(
            core.snapshot(),
            Snapshot {
                is_locked: true,
                summary: None,
                item_count: 0,
            }
        );
        assert!(core.entries().is_empty());
    }

    #[test]
    fn wrong_non_empty_password_keeps_core_locked() {
        let mut core = Core::new();

        assert_eq!(
            core.unlock("wrong-password"),
            Err(UnlockError::InvalidPassword)
        );
        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn correct_password_unlocks_and_exposes_demo_summary() {
        let mut core = Core::new();

        assert_eq!(core.unlock("correct horse"), Ok(()));
        assert_eq!(core.snapshot().summary, Some("Vault"));
        assert_eq!(core.snapshot().item_count, 3);
        assert_eq!(
            core.entries(),
            vec![
                EntrySummary {
                    title: "GitHub",
                    username: "octocat@example.com",
                },
                EntrySummary {
                    title: "Proton Mail",
                    username: "octocat@proton.me",
                },
                EntrySummary {
                    title: "Example Bank",
                    username: "octocat",
                },
            ]
        );
    }

    #[test]
    fn lock_returns_core_to_locked_state() {
        let mut core = Core::new();
        core.unlock("correct horse").unwrap();

        core.lock();

        assert!(core.snapshot().is_locked);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn locked_snapshot_and_entries_do_not_expose_sensitive_data() {
        let mut core = Core::new();
        core.unlock("correct horse").unwrap();
        core.lock();

        let snapshot = core.snapshot();
        assert_eq!(snapshot.summary, None);
        assert_eq!(snapshot.item_count, 0);
        assert!(core.entries().is_empty());
    }

    #[test]
    fn empty_password_is_rejected_and_keeps_core_locked() {
        let mut core = Core::new();

        assert_eq!(core.unlock(""), Err(UnlockError::EmptyPassword));
        assert!(core.snapshot().is_locked);
    }
}
