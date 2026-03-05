use crate::model::milestone::MilestoneMap;
use crate::model::policy::GatesPolicy;
use crate::model::project::ProjectMap;
use crate::model::traceability::TraceabilityMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Answering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Status,
    Contracts,
    Plan,
    Gates,
    Constraints,
    Questions,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Status,
            Tab::Contracts,
            Tab::Plan,
            Tab::Gates,
            Tab::Constraints,
            Tab::Questions,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Tab::Status => "Status",
            Tab::Contracts => "Contracts",
            Tab::Plan => "Plan",
            Tab::Gates => "Gates",
            Tab::Constraints => "Constraints",
            Tab::Questions => "Questions",
        }
    }

    pub fn next(&self) -> Tab {
        let tabs = Self::all();
        let idx = tabs.iter().position(|t| t == self).unwrap_or(0);
        tabs[(idx + 1) % tabs.len()]
    }

    pub fn prev(&self) -> Tab {
        let tabs = Self::all();
        let idx = tabs.iter().position(|t| t == self).unwrap_or(0);
        if idx == 0 {
            tabs[tabs.len() - 1]
        } else {
            tabs[idx - 1]
        }
    }
}

pub struct App {
    pub running: bool,
    pub current_tab: Tab,
    pub project_root: PathBuf,
    pub project: Option<ProjectMap>,
    pub milestones: Option<MilestoneMap>,
    pub gates_policy: Option<GatesPolicy>,
    pub traceability: Option<TraceabilityMap>,
    pub selected_index: usize,
    scroll_limit: usize,
    pub input_mode: InputMode,
    pub input_buffer: String,
    /// Index of the question being answered (within Questions tab)
    pub editing_question: Option<usize>,
    /// Status message shown briefly after an action
    pub status_message: Option<String>,
    /// Gate command being edited (gate index)
    pub editing_gate_command: Option<usize>,
    /// Gate cwd being edited (gate index)
    pub editing_gate_cwd: Option<usize>,
}

impl App {
    pub fn new(root: &Path) -> Self {
        let mut app = App {
            running: true,
            current_tab: Tab::Status,
            project_root: root.to_path_buf(),
            project: None,
            milestones: None,
            gates_policy: None,
            traceability: None,
            selected_index: 0,
            scroll_limit: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            editing_question: None,
            status_message: None,
            editing_gate_command: None,
            editing_gate_cwd: None,
        };
        app.reload();
        app
    }

    pub fn reload(&mut self) {
        let root = &self.project_root;
        self.project = ProjectMap::load(&root.join("project.yaml")).ok();
        self.milestones = MilestoneMap::load(&root.join("milestones.yaml")).ok();

        if let Some(ref p) = self.project {
            self.gates_policy =
                GatesPolicy::load(&root.join(&p.paths.validation.gates_policy)).ok();

            // Load milestone traceability
            if let Some(ref ms) = self.milestones {
                if let Some(ref current) = ms.current {
                    let ms_trace = root
                        .join("human/milestones")
                        .join(&current.id)
                        .join("traceability.yaml");
                    if ms_trace.exists() {
                        self.traceability = TraceabilityMap::load(&ms_trace).ok();
                    }
                }
            }
        } else {
            self.gates_policy = None;
            self.traceability = None;
        }
        self.selected_index = self.selected_index.min(self.scroll_limit);
    }

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
        self.selected_index = 0;
    }

    pub fn prev_tab(&mut self) {
        self.current_tab = self.current_tab.prev();
        self.selected_index = 0;
    }

    pub fn scroll_down(&mut self) {
        let next_index = self.selected_index.saturating_add(1);
        self.selected_index = next_index.min(self.scroll_limit);
    }

    pub fn scroll_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn set_scroll_limit(&mut self, limit: usize) {
        self.scroll_limit = limit;
        self.selected_index = self.selected_index.min(self.scroll_limit);
    }

    /// Cancel input mode.
    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.editing_question = None;
        self.input_buffer.clear();
    }

    /// Toggle enabled/disabled for the selected gate.
    pub fn toggle_gate(&mut self) {
        if self.current_tab != Tab::Gates {
            return;
        }
        if let Some(ref mut policy) = self.gates_policy {
            if let Some(gate) = policy.gates.get_mut(self.selected_index) {
                gate.enabled = !gate.enabled;
                let msg = format!(
                    "{} {}",
                    gate.id,
                    if gate.enabled { "enabled" } else { "disabled" }
                );
                self.save_gates_policy();
                self.status_message = Some(msg);
            }
        }
    }

    /// Start editing the command for the selected gate.
    pub fn start_editing_gate_command(&mut self) {
        if self.current_tab != Tab::Gates {
            return;
        }
        if let Some(ref policy) = self.gates_policy {
            if self.selected_index < policy.gates.len() {
                let existing = policy.gates[self.selected_index]
                    .command
                    .clone()
                    .unwrap_or_default();
                self.editing_gate_command = Some(self.selected_index);
                self.input_buffer = existing;
                self.input_mode = InputMode::Answering;
            }
        }
    }

    /// Submit the gate command being edited.
    pub fn submit_gate_command(&mut self) {
        if let Some(idx) = self.editing_gate_command {
            let cmd = self.input_buffer.trim().to_string();
            if let Some(ref mut policy) = self.gates_policy {
                if let Some(gate) = policy.gates.get_mut(idx) {
                    if cmd.is_empty() {
                        gate.command = None;
                        self.status_message = Some(format!("{} command cleared", gate.id));
                    } else {
                        gate.command = Some(cmd);
                        self.status_message = Some(format!("{} command set", gate.id));
                    }
                    self.save_gates_policy();
                }
            }
        }
        self.input_mode = InputMode::Normal;
        self.editing_gate_command = None;
        self.input_buffer.clear();
    }

    /// Start editing the cwd for the selected gate.
    pub fn start_editing_gate_cwd(&mut self) {
        if self.current_tab != Tab::Gates {
            return;
        }
        if let Some(ref policy) = self.gates_policy {
            if self.selected_index < policy.gates.len() {
                let existing = policy.gates[self.selected_index]
                    .cwd
                    .clone()
                    .unwrap_or_default();
                self.editing_gate_cwd = Some(self.selected_index);
                self.input_buffer = existing;
                self.input_mode = InputMode::Answering;
            }
        }
    }

    /// Submit the gate cwd being edited.
    pub fn submit_gate_cwd(&mut self) {
        if let Some(idx) = self.editing_gate_cwd {
            let cwd = self.input_buffer.trim().to_string();
            if let Some(ref mut policy) = self.gates_policy {
                if let Some(gate) = policy.gates.get_mut(idx) {
                    if cwd.is_empty() || cwd == "." {
                        gate.cwd = None;
                        self.status_message = Some(format!("{} cwd cleared", gate.id));
                    } else {
                        gate.cwd = Some(cwd);
                        self.status_message = Some(format!("{} cwd set", gate.id));
                    }
                    self.save_gates_policy();
                }
            }
        }
        self.input_mode = InputMode::Normal;
        self.editing_gate_cwd = None;
        self.input_buffer.clear();
    }

    /// Delete the selected gate.
    pub fn delete_gate(&mut self) {
        if self.current_tab != Tab::Gates {
            return;
        }
        if let Some(ref mut policy) = self.gates_policy {
            if self.selected_index < policy.gates.len() {
                let gate = policy.gates.remove(self.selected_index);
                let msg = format!("{} deleted", gate.id);
                self.save_gates_policy();
                self.status_message = Some(msg);
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
        }
    }

    /// Clear command for the selected gate.
    pub fn clear_gate_command(&mut self) {
        if self.current_tab != Tab::Gates {
            return;
        }
        if let Some(ref mut policy) = self.gates_policy {
            if let Some(gate) = policy.gates.get_mut(self.selected_index) {
                if gate.command.is_some() {
                    gate.command = None;
                    let msg = format!("{} command cleared", gate.id);
                    self.save_gates_policy();
                    self.status_message = Some(msg);
                }
            }
        }
    }

    /// Save gates policy to disk.
    fn save_gates_policy(&self) {
        if let (Some(ref policy), Some(ref project)) = (&self.gates_policy, &self.project) {
            let path = self
                .project_root
                .join(&project.paths.validation.gates_policy);
            let _ = policy.save(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App {
            running: true,
            current_tab: Tab::Status,
            project_root: PathBuf::from("/tmp/fake"),
            project: None,
            milestones: None,
            gates_policy: None,
            traceability: None,
            selected_index: 0,
            scroll_limit: 0,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            editing_question: None,
            status_message: None,
            editing_gate_command: None,
            editing_gate_cwd: None,
        }
    }

    #[test]
    fn tab_next_wraps() {
        assert_eq!(Tab::Questions.next(), Tab::Status);
    }

    #[test]
    fn tab_prev_wraps() {
        assert_eq!(Tab::Status.prev(), Tab::Questions);
    }

    #[test]
    fn tab_next_prev_roundtrip() {
        for &tab in Tab::all() {
            assert_eq!(tab.next().prev(), tab);
        }
    }

    #[test]
    fn tab_all_count() {
        assert_eq!(Tab::all().len(), 6);
    }

    #[test]
    fn tab_titles() {
        for &tab in Tab::all() {
            assert!(!tab.title().is_empty());
        }
    }

    #[test]
    fn scroll_up_saturates_at_zero() {
        let mut app = test_app();
        app.selected_index = 0;
        app.scroll_up();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn scroll_down_without_content_stays_at_zero() {
        let mut app = test_app();
        app.selected_index = 0;
        app.scroll_down();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn set_scroll_limit_clamps_selection() {
        let mut app = test_app();
        app.selected_index = 5;
        app.set_scroll_limit(2);
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn scroll_down_stops_at_scroll_limit() {
        let mut app = test_app();
        app.set_scroll_limit(2);
        app.scroll_down();
        app.scroll_down();
        app.scroll_down();
        assert_eq!(app.selected_index, 2);
    }

    #[test]
    fn scroll_limit_can_expand_again() {
        let mut app = test_app();
        app.set_scroll_limit(1);
        app.scroll_down();
        app.set_scroll_limit(3);
        app.scroll_down();
        app.scroll_down();
        assert_eq!(app.selected_index, 3);
    }

    #[test]
    fn quit_sets_running_false() {
        let mut app = test_app();
        assert!(app.running);
        app.quit();
        assert!(!app.running);
    }

    #[test]
    fn cancel_input_resets_state() {
        let mut app = test_app();
        app.input_mode = InputMode::Answering;
        app.input_buffer = "some text".to_string();
        app.editing_question = Some(0);
        app.cancel_input();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.input_buffer.is_empty());
        assert!(app.editing_question.is_none());
    }

    #[test]
    fn toggle_gate_wrong_tab_noop() {
        let mut app = test_app();
        app.current_tab = Tab::Status;
        app.toggle_gate();
        assert!(app.status_message.is_none());
    }

    fn app_with_gates() -> App {
        use crate::model::policy::{Gate, GatesPolicy};
        let mut app = test_app();
        app.current_tab = Tab::Gates;
        app.gates_policy = Some(GatesPolicy {
            version: "1.0.0".to_string(),
            policy_id: "TEST".to_string(),
            description: None,
            release_policy: None,
            gates: vec![
                Gate {
                    id: "GATE-001".to_string(),
                    gate_type: "contract".to_string(),
                    mandatory: true,
                    enabled: true,
                    pass_criteria: None,
                    command: None,
                    cwd: None,
                    tools: None,
                },
                Gate {
                    id: "GATE-002".to_string(),
                    gate_type: "test".to_string(),
                    mandatory: false,
                    enabled: false,
                    pass_criteria: None,
                    command: Some("echo hello".to_string()),
                    cwd: None,
                    tools: None,
                },
            ],
        });
        app
    }

    #[test]
    fn toggle_gate_flips_enabled() {
        let mut app = app_with_gates();
        app.selected_index = 0;
        assert!(app.gates_policy.as_ref().unwrap().gates[0].enabled);
        app.toggle_gate();
        assert!(!app.gates_policy.as_ref().unwrap().gates[0].enabled);
        assert!(app.status_message.as_ref().unwrap().contains("disabled"));
    }

    #[test]
    fn toggle_gate_out_of_bounds_noop() {
        let mut app = app_with_gates();
        app.selected_index = 99;
        app.toggle_gate();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn start_editing_gate_command_enters_answering() {
        let mut app = app_with_gates();
        app.selected_index = 1;
        app.start_editing_gate_command();
        assert_eq!(app.input_mode, InputMode::Answering);
        assert_eq!(app.editing_gate_command, Some(1));
        assert_eq!(app.input_buffer, "echo hello");
    }

    #[test]
    fn start_editing_gate_command_wrong_tab_noop() {
        let mut app = app_with_gates();
        app.current_tab = Tab::Contracts;
        app.start_editing_gate_command();
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.editing_gate_command.is_none());
    }

    #[test]
    fn submit_gate_command_sets_command() {
        let mut app = app_with_gates();
        app.editing_gate_command = Some(0);
        app.input_buffer = "cargo test".to_string();
        app.input_mode = InputMode::Answering;
        app.submit_gate_command();
        assert_eq!(
            app.gates_policy.as_ref().unwrap().gates[0]
                .command
                .as_deref(),
            Some("cargo test")
        );
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.status_message.as_ref().unwrap().contains("command set"));
    }

    #[test]
    fn submit_gate_command_empty_clears() {
        let mut app = app_with_gates();
        app.editing_gate_command = Some(1);
        app.input_buffer = "  ".to_string();
        app.submit_gate_command();
        assert!(app.gates_policy.as_ref().unwrap().gates[1]
            .command
            .is_none());
        assert!(app
            .status_message
            .as_ref()
            .unwrap()
            .contains("command cleared"));
    }

    #[test]
    fn start_editing_gate_cwd_enters_answering() {
        let mut app = app_with_gates();
        app.selected_index = 0;
        app.start_editing_gate_cwd();
        assert_eq!(app.input_mode, InputMode::Answering);
        assert_eq!(app.editing_gate_cwd, Some(0));
        assert_eq!(app.input_buffer, ""); // cwd was None
    }

    #[test]
    fn submit_gate_cwd_sets_value() {
        let mut app = app_with_gates();
        app.editing_gate_cwd = Some(0);
        app.input_buffer = "llm/src".to_string();
        app.submit_gate_cwd();
        assert_eq!(
            app.gates_policy.as_ref().unwrap().gates[0].cwd.as_deref(),
            Some("llm/src")
        );
        assert!(app.status_message.as_ref().unwrap().contains("cwd set"));
    }

    #[test]
    fn submit_gate_cwd_dot_clears() {
        let mut app = app_with_gates();
        app.editing_gate_cwd = Some(0);
        app.input_buffer = ".".to_string();
        app.submit_gate_cwd();
        assert!(app.gates_policy.as_ref().unwrap().gates[0].cwd.is_none());
        assert!(app.status_message.as_ref().unwrap().contains("cwd cleared"));
    }

    #[test]
    fn clear_gate_command_removes_existing() {
        let mut app = app_with_gates();
        app.selected_index = 1; // GATE-002 has command
        app.clear_gate_command();
        assert!(app.gates_policy.as_ref().unwrap().gates[1]
            .command
            .is_none());
        assert!(app
            .status_message
            .as_ref()
            .unwrap()
            .contains("command cleared"));
    }

    #[test]
    fn clear_gate_command_noop_when_no_command() {
        let mut app = app_with_gates();
        app.selected_index = 0; // GATE-001 has no command
        app.clear_gate_command();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn clear_gate_command_wrong_tab_noop() {
        let mut app = app_with_gates();
        app.current_tab = Tab::Plan;
        app.selected_index = 1;
        app.clear_gate_command();
        assert!(app.gates_policy.as_ref().unwrap().gates[1]
            .command
            .is_some());
    }

    #[test]
    fn next_tab_resets_scroll() {
        let mut app = test_app();
        app.set_scroll_limit(10);
        app.selected_index = 5;
        app.next_tab();
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.current_tab, Tab::Contracts);
    }

    #[test]
    fn prev_tab_resets_scroll() {
        let mut app = test_app();
        app.set_scroll_limit(10);
        app.selected_index = 5;
        app.prev_tab();
        assert_eq!(app.selected_index, 0);
        assert_eq!(app.current_tab, Tab::Questions);
    }

    fn fixture_root() -> PathBuf {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(manifest).join("tests/fixtures/milestone-project")
    }

    #[test]
    fn app_new_with_valid_fixture() {
        let root = fixture_root();
        let app = App::new(&root);
        assert!(app.running);
        assert!(app.project.is_some());
        assert!(app.milestones.is_some());
        assert!(app.gates_policy.is_some());
    }

    #[test]
    fn app_new_with_missing_dir() {
        let app = App::new(std::path::Path::new("/tmp/nonexistent-hlv-test"));
        assert!(app.project.is_none());
        assert!(app.milestones.is_none());
        assert!(app.gates_policy.is_none());
    }

    #[test]
    fn reload_reloads_data() {
        let root = fixture_root();
        let mut app = App::new(&root);
        let proj_name = app.project.as_ref().unwrap().project.clone();
        app.project = None;
        app.reload();
        assert_eq!(app.project.as_ref().unwrap().project, proj_name);
    }
}
