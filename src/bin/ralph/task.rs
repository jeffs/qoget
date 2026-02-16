use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const TASK_DIR: &str = "var/tasks";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pending,
    InProgress,
    Done,
    Failed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Bug,
    Feature,
}

// Variant order determines BTreeMap key ordering:
// Design < Reproduce < Test < Fix < Impl < Verify
// Bug tasks use {Reproduce, Test, Fix, Verify} — correct order.
// Feature tasks use {Design, Test, Impl, Verify} — correct order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
    Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Design,
    Reproduce,
    Test,
    Fix,
    #[serde(rename = "impl")]
    Impl,
    Verify,
}

impl std::fmt::Display for Stage {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Stage::Design => write!(f, "design"),
            Stage::Reproduce => write!(f, "reproduce"),
            Stage::Test => write!(f, "test"),
            Stage::Fix => write!(f, "fix"),
            Stage::Impl => write!(f, "impl"),
            Stage::Verify => write!(f, "verify"),
        }
    }
}

impl TaskType {
    pub fn stages(self) -> &'static [Stage] {
        match self {
            TaskType::Bug => &[
                Stage::Reproduce,
                Stage::Test,
                Stage::Fix,
                Stage::Verify,
            ],
            TaskType::Feature => &[
                Stage::Design,
                Stage::Test,
                Stage::Impl,
                Stage::Verify,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageState {
    pub status: Status,
    pub change_id: Option<String>,
    #[serde(default)]
    pub retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub priority: u32,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub status: Status,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    pub stages: BTreeMap<Stage, StageState>,
    #[serde(default)]
    pub context_files: Vec<String>,
    pub error: Option<String>,
    /// When true, Reproduce and Test stages run without
    /// the dead proxy, allowing upstream API access.
    #[serde(default)]
    pub allow_network: bool,
}

impl Task {
    pub fn path_for_id(id: &str) -> PathBuf {
        Path::new(TASK_DIR).join(format!("{id}.json"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| {
                format!("reading {}", path.display())
            })?;
        serde_json::from_str(&contents).with_context(|| {
            format!("parsing {}", path.display())
        })
    }

    pub fn load_all() -> Result<Vec<Self>> {
        let dir = Path::new(TASK_DIR);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "json") {
                tasks.push(Self::load(&path)?);
            }
        }
        tasks.sort_by_key(|t| t.priority);
        Ok(tasks)
    }

    /// Atomic write: temp file + rename.
    pub fn save(&self) -> Result<()> {
        let path = Self::path_for_id(&self.id);
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, &json).with_context(|| {
            format!("writing {}", tmp.display())
        })?;
        std::fs::rename(&tmp, &path).with_context(|| {
            format!(
                "renaming {} -> {}",
                tmp.display(),
                path.display()
            )
        })
    }

    pub fn is_runnable(&self, all_tasks: &[Task]) -> bool {
        if matches!(self.status, Status::Done | Status::Failed) {
            return false;
        }
        self.blockers.iter().all(|bid| {
            all_tasks
                .iter()
                .find(|t| t.id == *bid)
                .is_some_and(|t| t.status == Status::Done)
        })
    }

    pub fn next_stage(&self) -> Option<Stage> {
        self.task_type.stages().iter().copied().find(|s| {
            self.stages.get(s).is_some_and(|ss| {
                matches!(
                    ss.status,
                    Status::Pending | Status::InProgress
                )
            })
        })
    }

    pub fn all_stages_done(&self) -> bool {
        self.task_type.stages().iter().all(|s| {
            self.stages
                .get(s)
                .is_some_and(|ss| ss.status == Status::Done)
        })
    }

    pub fn set_stage_status(
        &mut self,
        stage: Stage,
        status: Status,
    ) {
        if let Some(ss) = self.stages.get_mut(&stage) {
            ss.status = status;
        }
    }

    pub fn set_stage_change_id(
        &mut self,
        stage: Stage,
        change_id: String,
    ) {
        if let Some(ss) = self.stages.get_mut(&stage) {
            ss.change_id = Some(change_id);
        }
    }

    pub fn clear_stage_change_id(
        &mut self,
        stage: Stage,
    ) {
        if let Some(ss) = self.stages.get_mut(&stage) {
            ss.change_id = None;
        }
    }

    pub fn stage_retries(&self, stage: Stage) -> u32 {
        self.stages
            .get(&stage)
            .map_or(0, |ss| ss.retries)
    }

    pub fn increment_stage_retries(
        &mut self,
        stage: Stage,
    ) {
        if let Some(ss) = self.stages.get_mut(&stage) {
            ss.retries += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_seed_task() {
        let json = r#"{
            "id": "001",
            "priority": 1,
            "type": "bug",
            "status": "pending",
            "title": "Bandcamp downloads return HTML",
            "description": "...",
            "blockers": [],
            "stage": null,
            "stages": {
                "reproduce": {
                    "status": "pending",
                    "change_id": null
                },
                "test": {
                    "status": "pending",
                    "change_id": null
                },
                "fix": {
                    "status": "pending",
                    "change_id": null
                },
                "verify": {
                    "status": "pending",
                    "change_id": null
                }
            },
            "context_files": ["src/bandcamp.rs"],
            "error": null,
            "retries": 0
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.id, "001");
        assert_eq!(task.task_type, TaskType::Bug);
        assert_eq!(task.status, Status::Pending);
        assert_eq!(
            task.next_stage(),
            Some(Stage::Reproduce)
        );
        assert!(!task.all_stages_done());
        assert!(task.is_runnable(&[]));
    }

    #[test]
    fn stage_progression() {
        let json = r#"{
            "id": "002",
            "priority": 1,
            "type": "bug",
            "status": "in-progress",
            "title": "Test",
            "description": "",
            "blockers": [],
            "stages": {
                "reproduce": {
                    "status": "done",
                    "change_id": "abc"
                },
                "test": {
                    "status": "pending",
                    "change_id": null
                },
                "fix": {
                    "status": "pending",
                    "change_id": null
                },
                "verify": {
                    "status": "pending",
                    "change_id": null
                }
            },
            "context_files": [],
            "error": null,
            "retries": 0
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        assert_eq!(task.next_stage(), Some(Stage::Test));
    }

    #[test]
    fn blocker_check() {
        let blocker = Task {
            id: "001".into(),
            priority: 1,
            task_type: TaskType::Bug,
            status: Status::Pending,
            title: "blocker".into(),
            description: String::new(),
            blockers: vec![],
            stages: BTreeMap::new(),
            context_files: vec![],
            error: None,
            allow_network: false,
        };
        let blocked = Task {
            id: "002".into(),
            priority: 1,
            task_type: TaskType::Bug,
            status: Status::Pending,
            title: "blocked".into(),
            description: String::new(),
            blockers: vec!["001".into()],
            stages: BTreeMap::new(),
            context_files: vec![],
            error: None,
            allow_network: false,
        };
        let all = vec![blocker.clone(), blocked.clone()];

        assert!(!blocked.is_runnable(&all));

        let mut done_blocker = blocker;
        done_blocker.status = Status::Done;
        let all = vec![done_blocker, blocked.clone()];
        assert!(blocked.is_runnable(&all));
    }

    #[test]
    fn roundtrip_serialization() {
        let json = r#"{
            "id": "001",
            "priority": 1,
            "type": "bug",
            "status": "pending",
            "title": "t",
            "description": "d",
            "stages": {
                "reproduce": {
                    "status": "pending",
                    "change_id": null
                },
                "test": {
                    "status": "pending",
                    "change_id": null
                },
                "fix": {
                    "status": "pending",
                    "change_id": null
                },
                "verify": {
                    "status": "pending",
                    "change_id": null
                }
            },
            "error": null
        }"#;

        let task: Task = serde_json::from_str(json).unwrap();
        let serialized =
            serde_json::to_string_pretty(&task).unwrap();
        let roundtrip: Task =
            serde_json::from_str(&serialized).unwrap();
        assert_eq!(task.id, roundtrip.id);
        assert_eq!(task.status, roundtrip.status);
        assert_eq!(
            task.next_stage(),
            roundtrip.next_stage()
        );
    }
}
