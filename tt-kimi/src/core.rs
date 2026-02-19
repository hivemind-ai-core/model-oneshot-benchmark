use crate::db::Database;
use crate::error::{Result, TaskError};
use crate::graph;
use crate::models::{
    Artifact, BlockedTaskInfo, DependencyInfo, NextTaskResult, OrderConflict, Status, Task,
    TaskDetail, WaitingOnInfo,
};
use std::collections::HashMap;
use std::path::Path;

/// Core business logic
pub struct TaskTracker {
    db: Database,
}

impl TaskTracker {
    /// Open the database in the current directory
    pub fn open() -> Result<Self> {
        let db = Database::open_current_dir()?;
        Ok(TaskTracker { db })
    }

    /// Open database at specific path
    pub fn open_at<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = Database::open(path)?;
        Ok(TaskTracker { db })
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> Result<bool> {
        self.db.is_initialized()
    }

    /// Initialize the database
    pub fn init(&self) -> Result<()> {
        self.db.init()
    }

    // ==================== Task Operations ====================

    /// Create a new task
    pub fn create_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<Task> {
        // Calculate manual_order based on positioning hints
        let manual_order = match (after_id, before_id) {
            (Some(after), Some(before)) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TaskError::TaskNotFound(after))?;
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TaskError::TaskNotFound(before))?;
                graph::calculate_midpoint(after_task.manual_order, before_task.manual_order)?
            }
            (Some(after), None) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TaskError::TaskNotFound(after))?;
                graph::calculate_order_after(after_task.manual_order)
            }
            (None, Some(before)) => {
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TaskError::TaskNotFound(before))?;
                graph::calculate_order_before(before_task.manual_order)
            }
            (None, None) => {
                let max_order = self.db.get_max_manual_order()?;
                graph::get_initial_order(max_order)
            }
        };

        self.db.create_task(title, description, dod, manual_order)
    }

    /// Get a task by ID
    pub fn get_task(&self, id: i64) -> Result<TaskDetail> {
        let task = self.db.get_task(id)?.ok_or(TaskError::TaskNotFound(id))?;
        self.load_task_details(task)
    }

    /// Get all tasks
    pub fn get_all_tasks(&self) -> Result<Vec<Task>> {
        self.db.get_all_tasks()
    }

    /// Update a task
    pub fn update_task(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<Option<&str>>,
        dod: Option<Option<&str>>,
    ) -> Result<TaskDetail> {
        let task = self.db.update_task(id, title, description, dod)?;
        self.load_task_details(task)
    }

    /// Delete is not supported in v1
    pub fn delete_task(&self, _id: i64) -> Result<()> {
        Err(TaskError::NotInitialized) // Placeholder - actually should return "not supported"
    }

    // ==================== Workflow Operations ====================

    /// Set the target task
    pub fn set_target(&self, id: i64) -> Result<()> {
        // Verify task exists
        if self.db.get_task(id)?.is_none() {
            return Err(TaskError::TaskNotFound(id));
        }
        self.db.set_config("target_id", &id.to_string())
    }

    /// Get the current target
    pub fn get_target(&self) -> Result<Option<i64>> {
        match self.db.get_config("target_id")? {
            Some(val) => Ok(Some(val.parse().map_err(|_| {
                TaskError::InvalidStatus("Invalid target_id in config".to_string())
            })?)),
            None => Ok(None),
        }
    }

    /// Clear the target
    pub fn clear_target(&self) -> Result<()> {
        self.db.delete_config("target_id")
    }

    /// Get the next task to work on
    pub fn get_next_task(&self, all: bool) -> Result<NextTaskResult> {
        let tasks_with_deps = if all {
            // Get all tasks
            let tasks = self.db.get_all_tasks()?;
            let mut result = Vec::new();
            for task in tasks {
                let deps = self.db.get_dependencies(task.id)?;
                let dep_ids: Vec<i64> = deps.into_iter().map(|d| d.depends_on).collect();
                result.push((task, dep_ids));
            }
            result
        } else {
            // Get target subgraph
            let target_id = match self.get_target()? {
                Some(id) => id,
                None => return Err(TaskError::NoTarget),
            };

            // Check if target itself is completed
            let target = self
                .db
                .get_task(target_id)?
                .ok_or(TaskError::TaskNotFound(target_id))?;
            if target.status == Status::Completed {
                return Ok(NextTaskResult::TargetReached { target_id });
            }

            self.db.get_target_subgraph_with_deps(target_id)?
        };

        // Filter out completed tasks from the list for processing
        let tasks_with_deps: Vec<(Task, Vec<i64>)> = tasks_with_deps
            .into_iter()
            .filter(|(t, _)| t.status != Status::Completed)
            .collect();

        if tasks_with_deps.is_empty() {
            if all {
                return Err(TaskError::NoTarget);
            } else if let Some(target_id) = self.get_target()? {
                return Ok(NextTaskResult::TargetReached { target_id });
            } else {
                return Err(TaskError::NoTarget);
            }
        }

        // Sort topologically
        let sort_result = graph::topological_sort(tasks_with_deps.clone())?;

        // Find first pending task with all deps completed
        let task_map: HashMap<i64, Task> = tasks_with_deps
            .iter()
            .map(|(t, _)| (t.id, t.clone()))
            .collect();
        let deps_map: HashMap<i64, Vec<i64>> = tasks_with_deps
            .into_iter()
            .map(|(t, deps)| (t.id, deps))
            .collect();

        let mut pending_tasks = Vec::new();
        let mut blocked_tasks = Vec::new();

        for task in &sort_result.ordered_tasks {
            if task.status == Status::Pending {
                let deps = deps_map.get(&task.id).cloned().unwrap_or_default();
                let dep_statuses: Vec<(i64, Status)> = deps
                    .iter()
                    .map(|&id| {
                        let status = task_map
                            .get(&id)
                            .map(|t| t.status)
                            .unwrap_or(Status::Completed);
                        (id, status)
                    })
                    .collect();

                match graph::can_start_task(task.id, &dep_statuses) {
                    None => {
                        // All deps met
                        let detail = self.load_task_details(task.clone())?;
                        return Ok(NextTaskResult::Task { task: detail });
                    }
                    Some(_) => {
                        pending_tasks.push(task.clone());
                    }
                }
            } else if task.status == Status::Blocked {
                blocked_tasks.push(task.clone());
            }
        }

        // If we get here, no pending task is ready
        // Check if all remaining are blocked
        if !blocked_tasks.is_empty() && pending_tasks.is_empty() {
            let blocked_info: Vec<BlockedTaskInfo> = blocked_tasks
                .into_iter()
                .map(|t| {
                    let deps = deps_map.get(&t.id).cloned().unwrap_or_default();
                    let waiting_on: Vec<WaitingOnInfo> = deps
                        .into_iter()
                        .filter_map(|id| {
                            task_map.get(&id).map(|task| WaitingOnInfo {
                                id: task.id,
                                title: task.title.clone(),
                                status: task.status,
                            })
                        })
                        .collect();
                    BlockedTaskInfo {
                        id: t.id,
                        title: t.title,
                        waiting_on,
                    }
                })
                .collect();
            return Ok(NextTaskResult::AllBlocked {
                tasks: blocked_info,
            });
        }

        // Check if target reached
        if let Some(target_id) = self.get_target()? {
            let target = self
                .db
                .get_task(target_id)?
                .ok_or(TaskError::TaskNotFound(target_id))?;
            if target.status == Status::Completed {
                return Ok(NextTaskResult::TargetReached { target_id });
            }
        }

        // No tasks available
        Err(TaskError::NoTarget)
    }

    /// Start a task
    pub fn start_task(&self, id: i64) -> Result<TaskDetail> {
        let task = self.db.get_task(id)?.ok_or(TaskError::TaskNotFound(id))?;

        // Idempotent: if already in progress, just return it
        if task.status == Status::InProgress {
            return self.load_task_details(task);
        }

        // Check if task is blocked
        if task.status == Status::Blocked {
            return Err(TaskError::TaskIsBlocked(id));
        }

        // Check if already completed
        if task.status == Status::Completed {
            return Err(TaskError::TaskAlreadyCompleted(id));
        }

        // Check for active task
        if let Some(active) = self.db.get_active_task()? {
            if active.id != id {
                return Err(TaskError::AnotherTaskActive(active.id));
            }
        }

        // Check dependencies
        let deps = self.db.get_dependency_statuses(id)?;
        let unmet: Vec<i64> = deps
            .iter()
            .filter(|(_, _, status)| *status != Status::Completed)
            .map(|(id, _, _)| *id)
            .collect();

        if !unmet.is_empty() {
            return Err(TaskError::UnmetDependencies { id, deps: unmet });
        }

        let task = self.db.set_task_status(id, Status::InProgress)?;
        self.load_task_details(task)
    }

    /// Stop the active task
    pub fn stop_task(&self) -> Result<TaskDetail> {
        let active = self.db.get_active_task()?.ok_or(TaskError::NoActiveTask)?;
        let task = self.db.set_task_status(active.id, Status::Pending)?;
        self.load_task_details(task)
    }

    /// Complete the active task
    pub fn complete_task(&self) -> Result<TaskDetail> {
        let active = self.db.get_active_task()?.ok_or(TaskError::NoActiveTask)?;

        // Check for DoD
        let task = self
            .db
            .get_task(active.id)?
            .ok_or(TaskError::TaskNotFound(active.id))?;
        if task
            .dod
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(TaskError::NoDod(active.id));
        }

        let task = self.db.set_task_status(active.id, Status::Completed)?;
        self.load_task_details(task)
    }

    /// Get the currently active task
    pub fn get_current_task(&self) -> Result<TaskDetail> {
        let active = self.db.get_active_task()?.ok_or(TaskError::NoActiveTask)?;
        self.load_task_details(active)
    }

    /// Block a task
    pub fn block_task(&self, id: i64) -> Result<TaskDetail> {
        let task = self.db.get_task(id)?.ok_or(TaskError::TaskNotFound(id))?;

        if task.status == Status::Blocked {
            return Err(TaskError::TaskNotBlocked(id)); // Already blocked
        }

        if task.status == Status::Completed {
            return Err(TaskError::TaskAlreadyCompleted(id));
        }

        let task = self.db.set_task_status(id, Status::Blocked)?;
        self.load_task_details(task)
    }

    /// Unblock a task
    pub fn unblock_task(&self, id: i64) -> Result<TaskDetail> {
        let task = self.db.get_task(id)?.ok_or(TaskError::TaskNotFound(id))?;

        if task.status != Status::Blocked {
            return Err(TaskError::TaskNotBlocked(id));
        }

        let task = self.db.set_task_status(id, Status::Pending)?;
        self.load_task_details(task)
    }

    // ==================== Dependency Operations ====================

    /// Add a dependency
    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        if task_id == depends_on {
            return Err(TaskError::SelfDependency);
        }

        // Verify both tasks exist
        if self.db.get_task(task_id)?.is_none() {
            return Err(TaskError::TaskNotFound(task_id));
        }
        if self.db.get_task(depends_on)?.is_none() {
            return Err(TaskError::TaskNotFound(depends_on));
        }

        // Get all existing dependencies
        let all_deps = self.db.get_all_dependencies()?;
        let dep_pairs: Vec<(i64, i64)> = all_deps
            .into_iter()
            .map(|d| (d.task_id, d.depends_on))
            .collect();

        // Check for cycle
        if let Some(cycle) = graph::would_create_cycle(&dep_pairs, task_id, depends_on) {
            return Err(TaskError::CycleDetected {
                from: task_id,
                to: depends_on,
                path: cycle,
            });
        }

        self.db.add_dependency(task_id, depends_on)
    }

    /// Remove a dependency
    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        self.db.remove_dependency(task_id, depends_on)
    }

    // ==================== Artifact Operations ====================

    /// Log an artifact for the active task
    pub fn log_artifact(&self, name: &str, file_path: &str) -> Result<Artifact> {
        let active = self.db.get_active_task()?.ok_or(TaskError::NoActiveTask)?;
        self.db.create_artifact(active.id, name, file_path)
    }

    /// Log an artifact for a specific task
    pub fn log_artifact_for_task(
        &self,
        task_id: i64,
        name: &str,
        file_path: &str,
    ) -> Result<Artifact> {
        // Verify task exists
        if self.db.get_task(task_id)?.is_none() {
            return Err(TaskError::TaskNotFound(task_id));
        }
        self.db.create_artifact(task_id, name, file_path)
    }

    /// Get artifacts for a task
    pub fn get_artifacts(&self, task_id: Option<i64>) -> Result<Vec<Artifact>> {
        let id = match task_id {
            Some(id) => id,
            None => {
                let active = self.db.get_active_task()?.ok_or(TaskError::NoActiveTask)?;
                active.id
            }
        };
        self.db.get_artifacts_for_task(id)
    }

    // ==================== Ordering Operations ====================

    /// Reorder a task
    pub fn reorder_task(
        &self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<Task> {
        if after_id.is_none() && before_id.is_none() {
            return Err(TaskError::MissingPositionHint);
        }

        let new_order = match (after_id, before_id) {
            (Some(after), Some(before)) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TaskError::TaskNotFound(after))?;
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TaskError::TaskNotFound(before))?;
                graph::calculate_midpoint(after_task.manual_order, before_task.manual_order)?
            }
            (Some(after), None) => {
                let after_task = self
                    .db
                    .get_task(after)?
                    .ok_or(TaskError::TaskNotFound(after))?;
                graph::calculate_order_after(after_task.manual_order)
            }
            (None, Some(before)) => {
                let before_task = self
                    .db
                    .get_task(before)?
                    .ok_or(TaskError::TaskNotFound(before))?;
                graph::calculate_order_before(before_task.manual_order)
            }
            (None, None) => unreachable!(),
        };

        self.db.update_manual_order(id, new_order)
    }

    /// Reindex all orders
    pub fn reindex(&self) -> Result<Vec<Task>> {
        let tasks = self.db.get_all_tasks_with_order()?;
        let reindexed = graph::reindex_orders(&tasks);

        for (id, order) in reindexed {
            self.db.update_manual_order(id, order)?;
        }

        self.db.get_all_tasks_with_order()
    }

    // ==================== List Operations ====================

    /// List tasks for a target or all tasks
    pub fn list_tasks(&self, all: bool) -> Result<(Vec<TaskDetail>, Vec<OrderConflict>)> {
        let tasks_with_deps = if all {
            let tasks = self.db.get_all_tasks()?;
            let mut result = Vec::new();
            for task in tasks {
                let deps = self.db.get_dependencies(task.id)?;
                let dep_ids: Vec<i64> = deps.into_iter().map(|d| d.depends_on).collect();
                result.push((task, dep_ids));
            }
            result
        } else {
            let target_id = match self.get_target()? {
                Some(id) => id,
                None => return Err(TaskError::NoTarget),
            };
            self.db.get_target_subgraph_with_deps(target_id)?
        };

        let sort_result = graph::topological_sort(tasks_with_deps)?;

        let mut details = Vec::new();
        for task in sort_result.ordered_tasks {
            details.push(self.load_task_details(task)?);
        }

        Ok((details, sort_result.order_conflicts))
    }

    /// Get tasks for the target subgraph
    pub fn get_target_tasks(&self, target_id: i64) -> Result<Vec<Task>> {
        self.db.get_full_target_subgraph(target_id)
    }

    // ==================== Helper Methods ====================

    fn load_task_details(&self, task: Task) -> Result<TaskDetail> {
        let deps = self.db.get_dependency_statuses(task.id)?;
        let dependencies: Vec<DependencyInfo> = deps
            .into_iter()
            .map(|(id, title, status)| DependencyInfo { id, title, status })
            .collect();

        let dependents = self.db.get_dependents(task.id)?;
        let artifacts = self.db.get_artifacts_for_task(task.id)?;

        Ok(TaskDetail {
            task,
            dependencies,
            dependents,
            artifacts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TaskTracker, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let tracker = TaskTracker::open_at(&db_path).unwrap();
        tracker.init().unwrap();
        (tracker, temp_dir)
    }

    #[test]
    fn test_create_task() {
        let (tracker, _temp) = setup();

        let task = tracker
            .create_task("Test Task", Some("Description"), Some("DoD"), None, None)
            .unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, Some("Description".to_string()));
        assert_eq!(task.dod, Some("DoD".to_string()));
        assert_eq!(task.status, Status::Pending);
    }

    #[test]
    fn test_start_task() {
        let (tracker, _temp) = setup();

        let task = tracker
            .create_task("Test", None, Some("DoD"), None, None)
            .unwrap();
        let detail = tracker.start_task(task.id).unwrap();

        assert_eq!(detail.task.status, Status::InProgress);
    }

    #[test]
    fn test_cannot_start_without_dod() {
        let (tracker, _temp) = setup();

        let task = tracker.create_task("Test", None, None, None, None).unwrap();
        tracker.start_task(task.id).unwrap();

        // Try to complete without DoD
        let result = tracker.complete_task();
        assert!(matches!(result, Err(TaskError::NoDod(_))));
    }

    #[test]
    fn test_dependency_cycle_detection() {
        let (tracker, _temp) = setup();

        let a = tracker.create_task("A", None, None, None, None).unwrap();
        let b = tracker.create_task("B", None, None, None, None).unwrap();
        let c = tracker.create_task("C", None, None, None, None).unwrap();

        tracker.add_dependency(b.id, a.id).unwrap();
        tracker.add_dependency(c.id, b.id).unwrap();

        // This would create a cycle
        let result = tracker.add_dependency(a.id, c.id);
        assert!(matches!(result, Err(TaskError::CycleDetected { .. })));
    }

    #[test]
    fn test_self_dependency_rejected() {
        let (tracker, _temp) = setup();

        let a = tracker.create_task("A", None, None, None, None).unwrap();
        let result = tracker.add_dependency(a.id, a.id);
        assert!(matches!(result, Err(TaskError::SelfDependency)));
    }

    #[test]
    fn test_target_workflow() {
        let (tracker, _temp) = setup();

        // Create tasks: A -> B -> C
        let a = tracker
            .create_task("A", None, Some("DoD A"), None, None)
            .unwrap();
        let b = tracker
            .create_task("B", None, Some("DoD B"), None, None)
            .unwrap();
        let c = tracker
            .create_task("C", None, Some("DoD C"), None, None)
            .unwrap();

        tracker.add_dependency(b.id, a.id).unwrap();
        tracker.add_dependency(c.id, b.id).unwrap();

        // Set C as target
        tracker.set_target(c.id).unwrap();

        // Next should return A
        let next = tracker.get_next_task(false).unwrap();
        match next {
            NextTaskResult::Task { task } => assert_eq!(task.task.id, a.id),
            _ => panic!("Expected task A"),
        }

        // Start and complete A
        tracker.start_task(a.id).expect("Failed to start A");
        tracker.complete_task().expect("Failed to complete A");

        // Next should return B
        let next = tracker
            .get_next_task(false)
            .expect("Failed to get next after A");
        match next {
            NextTaskResult::Task { task } => assert_eq!(task.task.id, b.id),
            _ => panic!("Expected task B"),
        }

        // Start and complete B
        tracker.start_task(b.id).expect("Failed to start B");
        tracker.complete_task().expect("Failed to complete B");

        // Verify target is still set
        let target = tracker.get_target().expect("Failed to get target");
        assert_eq!(target, Some(c.id), "Target should still be set to C");

        // Next should return C
        let next = tracker.get_next_task(false).unwrap();
        match next {
            NextTaskResult::Task { task } => assert_eq!(task.task.id, c.id),
            _ => panic!("Expected task C"),
        }

        // Start and complete C
        tracker.start_task(c.id).unwrap();
        tracker.complete_task().unwrap();

        // Target should be reached
        let next = tracker.get_next_task(false).unwrap();
        match next {
            NextTaskResult::TargetReached { target_id } => assert_eq!(target_id, c.id),
            _ => panic!("Expected target reached"),
        }
    }

    #[test]
    fn test_block_and_unblock() {
        let (tracker, _temp) = setup();

        let task = tracker.create_task("Test", None, None, None, None).unwrap();

        // Block
        let detail = tracker.block_task(task.id).unwrap();
        assert_eq!(detail.task.status, Status::Blocked);

        // Unblock
        let detail = tracker.unblock_task(task.id).unwrap();
        assert_eq!(detail.task.status, Status::Pending);
    }

    #[test]
    fn test_single_active_task() {
        let (tracker, _temp) = setup();

        let a = tracker
            .create_task("A", None, Some("DoD"), None, None)
            .unwrap();
        let b = tracker
            .create_task("B", None, Some("DoD"), None, None)
            .unwrap();

        tracker.start_task(a.id).unwrap();

        // Cannot start B while A is active
        let result = tracker.start_task(b.id);
        assert!(matches!(result, Err(TaskError::AnotherTaskActive(id)) if id == a.id));
    }

    #[test]
    fn test_unmet_dependencies() {
        let (tracker, _temp) = setup();

        let a = tracker
            .create_task("A", None, Some("DoD"), None, None)
            .unwrap();
        let b = tracker
            .create_task("B", None, Some("DoD"), None, None)
            .unwrap();

        tracker.add_dependency(b.id, a.id).unwrap();

        // Cannot start B without completing A
        let result = tracker.start_task(b.id);
        assert!(
            matches!(result, Err(TaskError::UnmetDependencies { id, deps }) if id == b.id && deps == vec![a.id])
        );
    }
}
