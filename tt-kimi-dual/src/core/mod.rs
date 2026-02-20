pub mod db;
pub mod error;
pub mod graph;
pub mod models;

use crate::core::db::Database;
use crate::core::error::{TTError, TTResult};
use crate::core::graph::{calculate_midpoint, default_order, order_after, order_before};
use crate::core::models::{
    Artifact, OrderConflict, Task, TaskDetail, TaskStatus, TaskWithDependencies,
};
use std::collections::HashSet;
use std::path::Path;

/// The main application core that coordinates all operations
pub struct AppCore {
    pub db: Database,
}

impl AppCore {
    /// Open the database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> TTResult<Self> {
        let db = Database::new(path)?;
        Ok(Self { db })
    }

    /// Initialize a new project (create database and artifacts directory)
    pub fn init<P: AsRef<Path>>(db_path: P, artifacts_dir: P) -> TTResult<Self> {
        if db_path.as_ref().exists() {
            return Err(TTError::AlreadyInitialized);
        }

        let db = Database::new(&db_path)?;
        db.init_schema()?;

        std::fs::create_dir_all(&artifacts_dir)?;

        Ok(Self { db })
    }

    // Task Management

    /// Create a new task with optional positioning hints
    pub fn add_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> TTResult<Task> {
        let order = match (after_id, before_id) {
            (Some(after), Some(before)) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TTError::TaskNotFound(after))?;
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TTError::TaskNotFound(before))?;
                calculate_midpoint(after_task.manual_order, before_task.manual_order)?
            }
            (Some(after), None) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TTError::TaskNotFound(after))?;
                order_after(after_task.manual_order)
            }
            (None, Some(before)) => {
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TTError::TaskNotFound(before))?;
                order_before(before_task.manual_order)
            }
            (None, None) => {
                let max = self.db.get_max_manual_order()?;
                default_order(max)
            }
        };

        self.db.create_task(title, description, dod, order)
    }

    /// Edit task fields
    pub fn edit_task(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        dod: Option<&str>,
    ) -> TTResult<Task> {
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))?;
        self.db.update_task_fields(id, title, description, dod)?;
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    /// Get a single task
    pub fn get_task(&self, id: i64) -> TTResult<Task> {
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    /// Get task with dependencies and dependents
    pub fn get_task_with_deps(&self, id: i64) -> TTResult<TaskWithDependencies> {
        let task = self.get_task(id)?;
        let dependencies = self.db.get_dependencies(id)?;
        let dependents = self.db.get_dependents(id)?;

        Ok(TaskWithDependencies {
            task,
            dependencies,
            dependents,
        })
    }

    /// Get full task detail
    pub fn get_task_detail(&self, id: i64) -> TTResult<TaskDetail> {
        let task = self.get_task(id)?;
        let dependencies = self.db.get_dependencies(id)?;
        let dependents = self.db.get_dependent_ids(id)?;
        let artifacts = self.db.get_artifacts_for_task(id)?;

        Ok(TaskDetail {
            task,
            dependencies,
            dependents,
            artifacts,
        })
    }

    // Workflow Operations

    /// Start a task (move to in_progress)
    pub fn start_task(&self, id: i64) -> TTResult<Task> {
        let task = self.get_task(id)?;

        // Idempotent: already in progress
        if task.is_in_progress() {
            return Ok(task);
        }

        // Must be pending
        if !task.is_pending() {
            return Err(TTError::TaskNotPending(id));
        }

        // Check for another active task
        if let Some(active) = self.db.get_active_task()? {
            if active.id != id {
                return Err(TTError::AnotherTaskActive(active.id));
            }
        }

        // Check all dependencies are completed
        let deps = self.db.get_dependencies(id)?;
        let unmet: Vec<i64> = deps
            .iter()
            .filter(|d| !d.is_completed())
            .map(|d| d.id)
            .collect();

        if !unmet.is_empty() {
            return Err(TTError::UnmetDependencies(id, unmet));
        }

        self.db.update_task_status(id, TaskStatus::InProgress)?;
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    /// Stop the active task (move back to pending)
    pub fn stop_task(&self) -> TTResult<Task> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        self.db.update_task_status(active.id, TaskStatus::Pending)?;
        self.db
            .get_task(active.id)?
            .ok_or(TTError::TaskNotFound(active.id))
    }

    /// Complete the active task
    pub fn complete_task(&self) -> TTResult<Task> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        // Check DoD exists
        if active.dod.is_none() || active.dod.as_ref().unwrap().trim().is_empty() {
            return Err(TTError::NoDod(active.id));
        }

        self.db
            .update_task_status(active.id, TaskStatus::Completed)?;
        self.db
            .get_task(active.id)?
            .ok_or(TTError::TaskNotFound(active.id))
    }

    /// Block a task
    pub fn block_task(&self, id: i64) -> TTResult<Task> {
        let task = self.get_task(id)?;

        if !task.is_pending() && !task.is_in_progress() {
            return Err(TTError::TaskNotPending(id));
        }

        self.db.update_task_status(id, TaskStatus::Blocked)?;
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    /// Unblock a task
    pub fn unblock_task(&self, id: i64) -> TTResult<Task> {
        let task = self.get_task(id)?;

        if !task.is_blocked() {
            return Err(TTError::InvalidStatus("Task is not blocked".to_string()));
        }

        self.db.update_task_status(id, TaskStatus::Pending)?;
        self.db.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    /// Get the currently active task
    pub fn get_active_task(&self) -> TTResult<Option<Task>> {
        self.db.get_active_task()
    }

    // Dependency Management

    /// Add a dependency between tasks
    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> TTResult<()> {
        // Verify both tasks exist
        self.get_task(task_id)?;
        self.get_task(depends_on)?;

        // Check for cycles
        let all_deps = self.db.get_all_dependencies()?;
        let existing: Vec<(i64, i64)> = all_deps
            .into_iter()
            .map(|d| (d.task_id, d.depends_on))
            .collect();

        if let Some(cycle) = crate::core::graph::detect_cycle(task_id, depends_on, &existing) {
            return Err(TTError::CycleDetected(task_id, depends_on, cycle));
        }

        self.db.add_dependency(task_id, depends_on)
    }

    /// Remove a dependency
    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> TTResult<()> {
        self.db.remove_dependency(task_id, depends_on)
    }

    // Ordering

    /// Reorder a task
    pub fn reorder_task(
        &self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> TTResult<f64> {
        if after_id.is_none() && before_id.is_none() {
            return Err(TTError::InvalidStatus(
                "Must specify --after or --before".to_string(),
            ));
        }

        let order = match (after_id, before_id) {
            (Some(after), Some(before)) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TTError::TaskNotFound(after))?;
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TTError::TaskNotFound(before))?;
                calculate_midpoint(after_task.manual_order, before_task.manual_order)?
            }
            (Some(after), None) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TTError::TaskNotFound(after))?;
                order_after(after_task.manual_order)
            }
            (None, Some(before)) => {
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TTError::TaskNotFound(before))?;
                order_before(before_task.manual_order)
            }
            (None, None) => unreachable!(),
        };

        self.db.update_manual_order(id, order)?;
        Ok(order)
    }

    /// Reindex all tasks to clean order values
    pub fn reindex(&self) -> TTResult<()> {
        let tasks = self.db.get_all_tasks()?;
        let new_orders = crate::core::graph::reindex_orders(&tasks);

        for (id, order) in new_orders {
            self.db.update_manual_order(id, order)?;
        }

        Ok(())
    }

    // Artifacts

    /// Log an artifact for the active task
    pub fn log_artifact(&self, name: &str, file_path: &str) -> TTResult<Artifact> {
        let active = self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?;

        self.db.create_artifact(active.id, name, file_path)
    }

    /// Get artifacts for a task
    pub fn get_artifacts(&self, task_id: Option<i64>) -> TTResult<Vec<Artifact>> {
        let id = match task_id {
            Some(id) => id,
            None => self.db.get_active_task()?.ok_or(TTError::NoActiveTask)?.id,
        };

        self.db.get_artifacts_for_task(id)
    }

    // Target Management

    /// Set the target task
    pub fn set_target(&self, task_id: i64) -> TTResult<()> {
        self.get_task(task_id)?; // Verify task exists
        self.db.set_target(task_id)
    }

    /// Get the current target
    pub fn get_target(&self) -> TTResult<Option<i64>> {
        self.db.get_target()
    }

    /// List tasks (target subgraph or all)
    pub fn list_tasks(&self, all: bool) -> TTResult<(Vec<Task>, Vec<OrderConflict>)> {
        if all {
            let tasks = self.db.get_all_tasks()?;
            let deps: Vec<(i64, i64)> = self
                .db
                .get_all_dependencies()?
                .into_iter()
                .map(|d| (d.task_id, d.depends_on))
                .collect();
            let (sorted, conflicts) = crate::core::graph::topological_sort(&tasks, &deps);
            Ok((sorted, conflicts))
        } else {
            let target_id = self.db.get_target()?.ok_or(TTError::NoTarget)?;
            let tasks = self.db.get_subgraph_tasks(target_id)?;

            if tasks.is_empty() {
                return Err(TTError::TargetReached(target_id));
            }

            let task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();
            let all_deps = self.db.get_all_dependencies()?;
            let deps: Vec<(i64, i64)> = all_deps
                .into_iter()
                .filter(|d| task_ids.contains(&d.task_id) && task_ids.contains(&d.depends_on))
                .map(|d| (d.task_id, d.depends_on))
                .collect();

            let (sorted, conflicts) = crate::core::graph::topological_sort(&tasks, &deps);
            Ok((sorted, conflicts))
        }
    }

    /// Get the next task to work on
    pub fn next_task(&self) -> TTResult<Option<Task>> {
        let target_id = self.db.get_target()?.ok_or(TTError::NoTarget)?;
        let tasks = self.db.get_subgraph_tasks(target_id)?;

        if tasks.is_empty() {
            return Err(TTError::TargetReached(target_id));
        }

        let task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();
        let all_deps = self.db.get_all_dependencies()?;
        let deps: Vec<(i64, i64)> = all_deps
            .into_iter()
            .filter(|d| task_ids.contains(&d.task_id) && task_ids.contains(&d.depends_on))
            .map(|d| (d.task_id, d.depends_on))
            .collect();

        let (sorted, _) = crate::core::graph::topological_sort(&tasks, &deps);

        // Find first pending task with all dependencies completed
        for task in &sorted {
            if task.is_pending() {
                let deps = self.db.get_dependencies(task.id)?;
                let all_completed = deps.iter().all(|d| d.is_completed());
                if all_completed {
                    return Ok(Some(task.clone()));
                }
            }
        }

        // Check if all remaining are blocked
        let blocked: Vec<i64> = sorted
            .iter()
            .filter(|t| t.is_blocked())
            .map(|t| t.id)
            .collect();

        if blocked.len() == sorted.len() {
            return Err(TTError::AllBlocked(blocked));
        }

        // No pending task ready (all have unmet dependencies)
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_core() -> (AppCore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let artifacts_path = temp_dir.path().join(".tt").join("artifacts");
        let core = AppCore::init(&db_path, &artifacts_path).unwrap();
        (core, temp_dir)
    }

    #[test]
    fn test_add_and_get_task() {
        let (core, _temp) = setup_test_core();

        let task = core
            .add_task("Test Task", Some("Description"), Some("DoD"), None, None)
            .unwrap();
        assert_eq!(task.title, "Test Task");
        assert!(task.is_pending());

        let fetched = core.get_task(task.id).unwrap();
        assert_eq!(fetched.title, "Test Task");
    }

    #[test]
    fn test_task_workflow() {
        let (core, _temp) = setup_test_core();

        // Create task with DoD
        let task = core
            .add_task("Test", None, Some("Definition of done"), None, None)
            .unwrap();

        // Start task
        let started = core.start_task(task.id).unwrap();
        assert!(started.is_in_progress());

        // Complete task
        let completed = core.complete_task().unwrap();
        assert!(completed.is_completed());
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_cannot_complete_without_dod() {
        let (core, _temp) = setup_test_core();

        let task = core.add_task("Test", None, None, None, None).unwrap();
        core.start_task(task.id).unwrap();

        let result = core.complete_task();
        assert!(matches!(result, Err(TTError::NoDod(_))));
    }

    #[test]
    fn test_dependencies() {
        let (core, _temp) = setup_test_core();

        let t1 = core
            .add_task("Task 1", None, Some("DoD"), None, None)
            .unwrap();
        let t2 = core
            .add_task("Task 2", None, Some("DoD"), None, None)
            .unwrap();

        // Add dependency: t2 depends on t1
        core.add_dependency(t2.id, t1.id).unwrap();

        // Try to start t2 before t1 is done
        let result = core.start_task(t2.id);
        assert!(matches!(result, Err(TTError::UnmetDependencies(_, _))));

        // Complete t1 first
        core.start_task(t1.id).unwrap();
        core.complete_task().unwrap();

        // Now t2 can be started
        let t2_started = core.start_task(t2.id).unwrap();
        assert!(t2_started.is_in_progress());
    }

    #[test]
    fn test_cycle_detection() {
        let (core, _temp) = setup_test_core();

        let t1 = core.add_task("Task 1", None, None, None, None).unwrap();
        let t2 = core.add_task("Task 2", None, None, None, None).unwrap();
        let t3 = core.add_task("Task 3", None, None, None, None).unwrap();

        core.add_dependency(t2.id, t1.id).unwrap();
        core.add_dependency(t3.id, t2.id).unwrap();

        // This would create a cycle: t1 -> t2 -> t3 -> t1
        let result = core.add_dependency(t1.id, t3.id);
        assert!(matches!(result, Err(TTError::CycleDetected(_, _, _))));
    }

    #[test]
    fn test_target_and_next() {
        let (core, _temp) = setup_test_core();

        let t1 = core
            .add_task("Task 1", None, Some("DoD"), None, None)
            .unwrap();
        let t2 = core
            .add_task("Task 2", None, Some("DoD"), None, None)
            .unwrap();

        // Add dependency: t2 depends on t1
        core.add_dependency(t2.id, t1.id).unwrap();

        // Set target
        core.set_target(t2.id).unwrap();

        // Get next task (should be t1)
        let next = core.next_task().unwrap().unwrap();
        assert_eq!(next.id, t1.id);

        // Complete t1
        core.start_task(t1.id).unwrap();
        core.complete_task().unwrap();

        // Next should be t2
        let next = core.next_task().unwrap().unwrap();
        assert_eq!(next.id, t2.id);

        // Complete t2
        core.start_task(t2.id).unwrap();
        core.complete_task().unwrap();

        // Target reached
        let result = core.next_task();
        assert!(matches!(result, Err(TTError::TargetReached(_))));
    }

    #[test]
    fn test_only_one_active_task() {
        let (core, _temp) = setup_test_core();

        let t1 = core
            .add_task("Task 1", None, Some("DoD"), None, None)
            .unwrap();
        let t2 = core
            .add_task("Task 2", None, Some("DoD"), None, None)
            .unwrap();

        core.start_task(t1.id).unwrap();

        let result = core.start_task(t2.id);
        assert!(matches!(result, Err(TTError::AnotherTaskActive(_))));
    }

    #[test]
    fn test_block_and_unblock() {
        let (core, _temp) = setup_test_core();

        let t1 = core.add_task("Task 1", None, None, None, None).unwrap();

        let blocked = core.block_task(t1.id).unwrap();
        assert!(blocked.is_blocked());

        let unblocked = core.unblock_task(t1.id).unwrap();
        assert!(unblocked.is_pending());
    }

    #[test]
    fn test_reorder() {
        let (core, _temp) = setup_test_core();

        let t1 = core.add_task("Task 1", None, None, None, None).unwrap();
        let t2 = core.add_task("Task 2", None, None, None, None).unwrap();
        let t3 = core.add_task("Task 3", None, None, None, None).unwrap();

        // Reorder t3 to be between t1 and t2
        core.reorder_task(t3.id, Some(t1.id), Some(t2.id)).unwrap();

        let t3_updated = core.get_task(t3.id).unwrap();
        assert!(t3_updated.manual_order > t1.manual_order);
        assert!(t3_updated.manual_order < t2.manual_order);
    }
}
