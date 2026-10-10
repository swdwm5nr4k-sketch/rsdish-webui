use std::path::PathBuf;

/// What a single member (cabinet) did during an action.
#[derive(Debug, Default, Clone)]
pub struct MemberReport {
    pub cabinet_abs_path: PathBuf,
    /// Files the plan selected.
    pub planned: usize,
    /// Files actually handled without error.
    pub done: usize,
    /// Human readable failures, empty when everything went well.
    pub failures: Vec<String>,
}

/// What a whole group did during an action.
#[derive(Debug, Default, Clone)]
pub struct ActionReport {
    pub members: Vec<MemberReport>,
}

impl ActionReport {
    pub fn planned(&self) -> usize {
        self.members.iter().map(|m| m.planned).sum()
    }

    pub fn done(&self) -> usize {
        self.members.iter().map(|m| m.done).sum()
    }

    pub fn failures(&self) -> Vec<String> {
        self.members
            .iter()
            .flat_map(|m| m.failures.iter().cloned())
            .collect()
    }
}
