//! Persistent record of active Worktree Swarms.
//!
//! Stored at `~/.config/tmx/state.json` so `tmx review` can find each agent's
//! worktree + branch long after the spawning process has exited.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentEntry {
    pub codename: String,
    pub branch: String,
    pub worktree: String,
    pub pane: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Swarm {
    pub session: String,
    pub repo_root: String,
    pub base_branch: String,
    pub task: String,
    pub agents: Vec<AgentEntry>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub swarms: BTreeMap<String, Swarm>,
}

impl State {
    /// Load state, tolerating a missing or corrupt file (returns default).
    pub fn load(path: &Path) -> State {
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => State::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize state: {e}"))?;
        fs::write(path, json).map_err(|e| format!("failed to write state: {e}"))
    }
}
