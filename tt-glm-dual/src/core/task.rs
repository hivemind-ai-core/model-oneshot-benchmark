//! Task operations for the tt task tracker.
//!
//! Implements all business logic for task management, workflow, dependencies,
//! artifacts, and ordering.

use crate::core::db::{Artifact, Db, Task, TaskWithDeps};
use crate::core::error::{Result, TTError};
use crate::core::graph;

/// Task manager for high-level operations.
pub struct TaskManager {
    db: Db,
}

impl TaskManager {
    /// Create a new task manager.
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Get a reference to the database.
    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Get a mutable reference to the database.
    pub fn db_mut(&mut self) -> &mut Db {
        &mut self.db
    }

    /// Add a new task.
    pub fn add_task(
        &mut self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<i64> {
        if after_id.is_none() && before_id.is_none() {
            let max_order = self.db.get_max_manual_order()?;
            let order = if max_order > 0.0 {
                max_order + 10.0
            } else {
                10.0
            };
            return self.db.create_task(title, description, dod, order);
        }

        let order = graph::calculate_midpoint(&self.db, after_id, before_id)?;
        self.db.create_task(title, description, dod, order)
    }

    /// Edit a task.
    pub fn edit_task(
        &mut self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        dod: Option<&str>,
    ) -> Result<()> {
        self.db.get_task(id)?; // Verify task exists
        self.db.update_task(id, title, description, dod)
    }

    /// Show a task with dependencies and dependents.
    pub fn show_task(&self, id: i64) -> Result<TaskWithDeps> {
        let task = self.db.get_task(id)?;
        let dependencies = self.db.get_dependencies(id)?;
        let dependents = self.db.get_dependents(id)?;
        let artifacts = self.db.get_artifacts(id)?;

        Ok(TaskWithDeps {
            task,
            dependencies,
            dependents,
            artifacts,
        })
    }

    /// List tasks (either all or in target subgraph).
    pub fn list_tasks(&self, all: bool) -> Result<Vec<TaskWithDeps>> {
        if all {
            self.db.get_tasks_with_deps()
        } else {
            let target_id = self.get_target()?;
            let tasks = self.db.get_incomplete_in_subgraph(target_id)?;
            let mut result = Vec::new();

            for task in tasks {
                let deps = self.db.get_dependencies(task.id)?;
                let dependents = self.db.get_dependents(task.id)?;
                let artifacts = self.db.get_artifacts(task.id)?;

                result.push(TaskWithDeps {
                    task,
                    dependencies: deps,
                    dependents,
                    artifacts,
                });
            }

            Ok(result)
        }
    }

    /// Set the target.
    pub fn set_target(&mut self, id: i64) -> Result<()> {
        self.db.get_task(id)?; // Verify task exists
        self.db.set_config("target_id", &id.to_string())
    }

    /// Get the target ID.
    pub fn get_target(&self) -> Result<i64> {
        let value = self.db.get_config("target_id")?.ok_or(TTError::NoTarget)?;
        value.parse().map_err(|_| TTError::NoTarget)
    }

    /// Get the next task to work on.
    pub fn next_task(&self) -> Result<TaskWithDeps> {
        let target_id = self.get_target()?;
        let subgraph_ids = self.db.get_target_subgraph(target_id)?;

        // Filter out completed tasks
        let incomplete: Vec<i64> = subgraph_ids
            .into_iter()
            .filter(|&id| match self.db.get_task(id) {
                Ok(task) => task.status != "completed",
                Err(_) => false,
            })
            .collect();

        if incomplete.is_empty() {
            return Err(TTError::TargetReached(target_id));
        }

        let sorted = graph::topological_sort(&self.db, &incomplete)?;

        // Find the first pending task with all deps completed
        for task_in_order in &sorted {
            if task_in_order.task.status == "pending" && task_in_order.all_deps_completed {
                return self.show_task(task_in_order.task.id);
            }
        }

        // All remaining tasks are blocked
        let blocked: Vec<i64> = sorted
            .iter()
            .filter(|t| t.task.status != "completed")
            .map(|t| t.task.id)
            .collect();

        Err(TTError::AllBlocked(blocked))
    }

    /// Start a task.
    pub fn start_task(&mut self, id: i64) -> Result<Task> {
        // Verify task exists and is pending
        let task = self.db.get_task(id)?;
        if task.status != "pending" && task.status != "in_progress" {
            return Err(TTError::TaskNotPending(id));
        }

        // Check if already in progress (idempotent)
        if task.status == "in_progress" {
            return Ok(task);
        }

        // Check for another active task
        if let Some(active) = self.db.get_active_task()? {
            return Err(TTError::AnotherTaskActive(active.id));
        }

        // Check dependencies
        let deps = self.db.get_dependencies(id)?;
        let unmet: Vec<i64> = deps
            .into_iter()
            .filter(|&dep_id| match self.db.get_task(dep_id) {
                Ok(dep_task) => dep_task.status != "completed",
                Err(_) => true,
            })
            .collect();

        if !unmet.is_empty() {
            return Err(TTError::UnmetDependencies(id, unmet));
        }

        // Update status
        self.db.update_task_status(id, "in_progress")?;
        self.db.get_task(id)
    }

    /// Stop the active task.
    pub fn stop_task(&mut self) -> Result<Task> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        self.db.update_task_status(active.id, "pending")?;
        self.db.get_task(active.id)
    }

    /// Complete the active task.
    pub fn complete_task(&mut self) -> Result<Task> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        // Check DoD
        if active.dod.is_none() || active.dod.as_ref().map(|s| s.trim()).unwrap_or("") == "" {
            return Err(TTError::NoDod(active.id));
        }

        self.db.update_task_status(active.id, "completed")?;
        self.db.get_task(active.id)
    }

    /// Block a task.
    pub fn block_task(&mut self, id: i64) -> Result<Task> {
        let task = self.db.get_task(id)?;

        match task.status.as_str() {
            "pending" | "in_progress" => {
                self.db.update_task_status(id, "blocked")?;
                self.db.get_task(id)
            }
            _ => Err(TTError::InvalidTransition(
                task.status.clone(),
                "blocked".to_string(),
            )),
        }
    }

    /// Unblock a task.
    pub fn unblock_task(&mut self, id: i64) -> Result<Task> {
        let task = self.db.get_task(id)?;

        if task.status != "blocked" {
            return Err(TTError::InvalidTransition(
                task.status.clone(),
                "pending".to_string(),
            ));
        }

        self.db.update_task_status(id, "pending")?;
        self.db.get_task(id)
    }

    /// Get the current active task.
    pub fn get_current_task(&self) -> Result<TaskWithDeps> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;
        self.show_task(active.id)
    }

    /// Add a dependency.
    pub fn add_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<()> {
        // Verify both tasks exist
        self.db.get_task(task_id)?;
        self.db.get_task(depends_on)?;

        // Check for cycles
        graph::check_cycle(&self.db, task_id, depends_on)?;

        self.db.add_dependency(task_id, depends_on)
    }

    /// Remove a dependency.
    pub fn remove_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<()> {
        self.db.get_task(task_id)?; // Verify task exists
        self.db.remove_dependency(task_id, depends_on)
    }

    /// Log an artifact for the active task.
    pub fn log_artifact(&mut self, name: &str, file_path: &str) -> Result<Artifact> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        let id = self.db.add_artifact(active.id, name, file_path)?;
        let artifacts = self.db.get_artifacts(active.id)?;
        Ok(artifacts.into_iter().find(|a| a.id == id).unwrap())
    }

    /// Get artifacts for a task.
    pub fn get_artifacts(&self, task_id: Option<i64>) -> Result<Vec<Artifact>> {
        let id = if let Some(tid) = task_id {
            tid
        } else {
            let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;
            active.id
        };

        self.db.get_artifacts(id)
    }

    /// Reorder a task.
    pub fn reorder_task(
        &mut self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<()> {
        if after_id.is_none() && before_id.is_none() {
            return Err(TTError::AfterOrBeforeRequired);
        }

        let order = graph::calculate_midpoint(&self.db, after_id, before_id)?;
        self.db.update_task_order(id, order)
    }

    /// Reindex all task orders.
    pub fn reindex(&mut self) -> Result<()> {
        self.db.reindex_orders()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn setup_manager() -> (TaskManager, NamedTempFile) {
        let temp = NamedTempFile::new().unwrap();
        let db = Db::open(temp.path()).unwrap();
        db.init_schema().unwrap();
        (TaskManager::new(db), temp)
    }

    #[test]
    fn test_add_task() {
        let (mut mgr, _) = setup_manager();
        let id = mgr
            .add_task("Test task", Some("Description"), Some("DoD"), None, None)
            .unwrap();
        assert!(id > 0);

        let task = mgr.db().get_task(id).unwrap();
        assert_eq!(task.title, "Test task");
    }

    #[test]
    fn test_start_task() {
        let (mut mgr, _) = setup_manager();
        let id = mgr
            .add_task("Test task", None, Some("DoD"), None, None)
            .unwrap();

        let task = mgr.start_task(id).unwrap();
        assert_eq!(task.status, "in_progress");
    }

    #[test]
    fn test_another_task_active() {
        let (mut mgr, _) = setup_manager();
        let id1 = mgr
            .add_task("Task 1", None, Some("DoD"), None, None)
            .unwrap();
        let id2 = mgr
            .add_task("Task 2", None, Some("DoD"), None, None)
            .unwrap();

        mgr.start_task(id1).unwrap();
        let result = mgr.start_task(id2);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_task_requires_dod() {
        let (mut mgr, _) = setup_manager();
        let id = mgr.add_task("Test task", None, None, None, None).unwrap();

        mgr.start_task(id).unwrap();
        let result = mgr.complete_task();
        assert!(result.is_err());
    }

    #[test]
    fn test_workflow() {
        let (mut mgr, _) = setup_manager();

        let id1 = mgr
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();
        let id2 = mgr
            .add_task("Task 2", None, Some("DoD2"), None, None)
            .unwrap();

        mgr.add_dependency(id2, id1).unwrap();
        mgr.set_target(id2).unwrap();

        // Should not be able to start id2 yet
        let result = mgr.start_task(id2);
        assert!(result.is_err());

        // Start and complete id1
        mgr.start_task(id1).unwrap();
        mgr.complete_task().unwrap();

        // Now id2 should be available
        let next = mgr.next_task().unwrap();
        assert_eq!(next.task.id, id2);
    }
}
