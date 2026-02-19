//! Task repository - high-level task operations.

use crate::core::config::{get_target, set_target};
use crate::core::dependency;
use crate::core::{Task, TaskStatus};
use crate::db::{schema::TaskRow, Connection};
use crate::error::{Error, Result};
use std::collections::{BinaryHeap, HashMap};

/// Task repository.
pub struct TaskRepository {
    conn: Connection,
}

impl TaskRepository {
    /// Open a new repository connection.
    pub fn open() -> Result<Self> {
        let conn = Connection::open_default()?;
        Ok(Self { conn })
    }

    /// Open an in-memory repository for testing.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(Self { conn })
    }

    /// Get the underlying connection.
    pub fn conn(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Create a new task.
    pub fn create_task(
        &mut self,
        title: String,
        description: Option<String>,
        dod: Option<String>,
        manual_order: f64,
    ) -> Result<Task> {
        self.conn.execute(
            "INSERT INTO tasks (title, description, dod, manual_order) VALUES (?, ?, ?, ?)",
            &[
                &title as &dyn rusqlite::ToSql,
                &description as &dyn rusqlite::ToSql,
                &dod as &dyn rusqlite::ToSql,
                &manual_order as &dyn rusqlite::ToSql,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task(id)
    }

    /// Get a task by ID.
    pub fn get_task(&mut self, id: i64) -> Result<Task> {
        let row = self.conn.query_row(
            "SELECT * FROM tasks WHERE id = ?",
            &[&id as &dyn rusqlite::ToSql],
            TaskRow::from_row,
        )?;

        Task::from_row(row)
    }

    /// Get all tasks.
    pub fn get_all_tasks(&mut self) -> Result<Vec<Task>> {
        let rows = self
            .conn
            .query("SELECT * FROM tasks ORDER BY manual_order", &[], |row| {
                TaskRow::from_row(row)
            })?;

        rows.into_iter().map(Task::from_row).collect()
    }

    /// Get the active task (status = in_progress).
    pub fn get_active_task(&mut self) -> Result<Task> {
        let row = self
            .conn
            .query_row(
                "SELECT * FROM tasks WHERE status = 'in_progress' LIMIT 1",
                &[],
                TaskRow::from_row,
            )
            .map_err(|_| Error::NoActiveTask)?;

        Task::from_row(row)
    }

    /// Update a task.
    pub fn update_task(
        &mut self,
        id: i64,
        title: Option<String>,
        description: Option<String>,
        dod: Option<String>,
    ) -> Result<Task> {
        let task = self.get_task(id)?;

        if task.status == TaskStatus::Completed {
            return Err(Error::TaskCompleted(id));
        }

        // Build the UPDATE dynamically based on what's provided
        if let Some(t) = title {
            self.conn.execute(
                "UPDATE tasks SET title = ?, last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
                &[&t as &dyn rusqlite::ToSql, &id as &dyn rusqlite::ToSql],
            )?;
        }
        if description.is_some() {
            self.conn.execute(
                "UPDATE tasks SET description = ?, last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
                &[&description as &dyn rusqlite::ToSql, &id as &dyn rusqlite::ToSql],
            )?;
        }
        if dod.is_some() {
            self.conn.execute(
                "UPDATE tasks SET dod = ?, last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
                &[&dod as &dyn rusqlite::ToSql, &id as &dyn rusqlite::ToSql],
            )?;
        }

        self.get_task(id)
    }

    /// Start a task (move to in_progress).
    pub fn start_task(&mut self, id: i64) -> Result<Task> {
        // Check if another task is already active
        if let Ok(active) = self.get_active_task() {
            if active.id == id {
                // Already active, return as no-op
                return self.get_task(id);
            }
            return Err(Error::AnotherTaskActive(active.id));
        }

        let task = self.get_task(id)?;

        // Check if task is in pending status
        if task.status != TaskStatus::Pending {
            return Err(Error::TaskNotPending(id));
        }

        // Check all dependencies are completed
        let deps = dependency::get_dependencies(&mut self.conn, id)?;
        let unmet: Vec<i64> = deps
            .into_iter()
            .filter(|&dep_id| {
                if let Ok(dep_task) = self.get_task(dep_id) {
                    dep_task.status != TaskStatus::Completed
                } else {
                    true
                }
            })
            .collect();

        if !unmet.is_empty() {
            return Err(Error::UnmetDependencies(
                id,
                crate::error::format_task_ids(&unmet),
            ));
        }

        // Update the task
        self.conn.execute(
            "UPDATE tasks SET status = 'in_progress', started_at = strftime('%Y-%m-%dT%H:%M:%S', 'now'), last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
            &[&id as &dyn rusqlite::ToSql],
        )?;

        self.get_task(id)
    }

    /// Stop the active task (move back to pending).
    pub fn stop_task(&mut self) -> Result<Task> {
        let task = self.get_active_task()?;

        self.conn.execute(
            "UPDATE tasks SET status = 'pending', last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
            &[&task.id as &dyn rusqlite::ToSql],
        )?;

        self.get_task(task.id)
    }

    /// Complete the active task.
    pub fn complete_task(&mut self) -> Result<Task> {
        let task = self.get_active_task()?;

        // Check DoD is set
        if task.dod.is_none() || task.dod.as_ref().is_none_or(|s| s.is_empty()) {
            return Err(Error::NoDod(task.id));
        }

        self.conn.execute(
            "UPDATE tasks SET status = 'completed', completed_at = strftime('%Y-%m-%dT%H:%M:%S', 'now'), last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
            &[&task.id as &dyn rusqlite::ToSql],
        )?;

        self.get_task(task.id)
    }

    /// Block a task.
    pub fn block_task(&mut self, id: i64) -> Result<Task> {
        let task = self.get_task(id)?;

        match task.status {
            TaskStatus::Pending | TaskStatus::InProgress => {
                self.conn.execute(
                    "UPDATE tasks SET status = 'blocked', last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
                    &[&id as &dyn rusqlite::ToSql],
                )?;
            }
            _ => {
                return Err(Error::InvalidTransition(
                    task.status.as_str().to_string(),
                    "blocked".to_string(),
                ))
            }
        }

        self.get_task(id)
    }

    /// Unblock a task.
    pub fn unblock_task(&mut self, id: i64) -> Result<Task> {
        let task = self.get_task(id)?;

        if task.status != TaskStatus::Blocked {
            return Err(Error::TaskNotBlocked(id));
        }

        self.conn.execute(
            "UPDATE tasks SET status = 'pending', last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
            &[&id as &dyn rusqlite::ToSql],
        )?;

        self.get_task(id)
    }

    /// Set the target.
    pub fn set_target(&mut self, id: i64) -> Result<()> {
        set_target(&mut self.conn, id)
    }

    /// Get the target.
    pub fn get_target(&mut self) -> Result<Option<i64>> {
        get_target(&mut self.conn)
    }

    /// Get the next task to work on.
    pub fn get_next_task(&mut self) -> Result<Task> {
        let target_id = self.get_target()?.ok_or(Error::NoTarget)?;

        // Get all tasks in the target subgraph
        let task_ids = self.get_target_subgraph(target_id)?;

        if task_ids.is_empty() {
            return Err(Error::TargetReached(target_id));
        }

        // Filter tasks that are ready to start (pending with all deps completed)
        let mut ready_tasks = Vec::new();
        for &task_id in &task_ids {
            if let Ok(task) = self.get_task(task_id) {
                if task.status == TaskStatus::Pending {
                    let deps = dependency::get_dependencies(&mut self.conn, task_id)?;
                    let all_complete = deps.iter().all(|&dep_id| {
                        self.get_task(dep_id)
                            .map(|t| t.status == TaskStatus::Completed)
                            .unwrap_or(false)
                    });
                    if all_complete {
                        ready_tasks.push(task);
                    }
                }
            }
        }

        if ready_tasks.is_empty() {
            // All remaining tasks are blocked
            return Err(Error::AllBlocked(
                task_ids
                    .into_iter()
                    .map(|id| format!("#{id}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        // Sort by manual_order
        ready_tasks.sort_by(|a, b| {
            a.manual_order
                .partial_cmp(&b.manual_order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(ready_tasks.into_iter().next().unwrap())
    }

    /// List tasks in the target subgraph.
    pub fn list_tasks(&mut self, all: bool) -> Result<Vec<Task>> {
        if all {
            return self.get_all_tasks();
        }

        let target_id = self.get_target()?.ok_or(Error::NoTarget)?;
        let task_ids = self.get_target_subgraph(target_id)?;
        let mut tasks = Vec::new();
        for id in task_ids {
            if let Ok(task) = self.get_task(id) {
                tasks.push(task);
            }
        }
        self.topological_sort(tasks)
    }

    /// Get the transitive dependency subgraph for a target.
    fn get_target_subgraph(&mut self, target_id: i64) -> Result<Vec<i64>> {
        // Recursive CTE to find all transitive dependencies
        let sql = "
            WITH RECURSIVE subgraph AS (
                SELECT id, status FROM tasks WHERE id = ?
                UNION
                SELECT t.id, t.status FROM tasks t
                INNER JOIN dependencies d ON t.id = d.task_id
                INNER JOIN subgraph s ON d.depends_on = s.id
                WHERE t.status != 'completed'
            )
            SELECT id FROM subgraph
        ";

        let ids = self
            .conn
            .query(sql, &[&target_id as &dyn rusqlite::ToSql], |row| row.get(0))?;
        Ok(ids)
    }

    /// Topological sort of tasks by dependency.
    fn topological_sort(&mut self, tasks: Vec<Task>) -> Result<Vec<Task>> {
        // Build adjacency list and in-degree map
        let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut in_degree: HashMap<i64, i32> = HashMap::new();
        let mut task_map: HashMap<i64, Task> = HashMap::new();

        for task in &tasks {
            task_map.insert(task.id, task.clone());
            adjacency.entry(task.id).or_default();
            in_degree.entry(task.id).or_insert(0);
        }

        // Populate dependencies (need to query from DB)
        for task in &tasks {
            let deps = dependency::get_dependencies(&mut self.conn, task.id)?;
            for &dep_id in &deps {
                adjacency.entry(dep_id).or_default().push(task.id);
                *in_degree.entry(task.id).or_insert(0) += 1;
            }
        }

        // Use min-heap based on manual_order
        let mut heap: BinaryHeap<OrderedTask> = BinaryHeap::new();

        for (&id, &degree) in &in_degree {
            if degree == 0 {
                if let Some(task) = task_map.get(&id) {
                    heap.push(OrderedTask(task.clone()));
                }
            }
        }

        let mut result = Vec::new();

        while let Some(ordered_task) = heap.pop() {
            let task_id = ordered_task.0.id;
            result.push(ordered_task.0);

            if let Some(deps) = adjacency.get(&task_id) {
                for &dep_id in deps {
                    if let Some(degree) = in_degree.get_mut(&dep_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            if let Some(task) = task_map.get(&dep_id) {
                                heap.push(OrderedTask(task.clone()));
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Reindex all manual_order values.
    pub fn reindex(&mut self) -> Result<()> {
        let tasks = self.get_all_tasks()?;
        for (i, task) in tasks.iter().enumerate() {
            let new_order = (i as i64 + 1) * 10;
            self.conn.execute(
                "UPDATE tasks SET manual_order = ? WHERE id = ?",
                &[
                    &new_order as &dyn rusqlite::ToSql,
                    &task.id as &dyn rusqlite::ToSql,
                ],
            )?;
        }
        Ok(())
    }
}

/// Wrapper for min-heap ordering by manual_order.
#[derive(Debug, Clone)]
struct OrderedTask(Task);

impl PartialEq for OrderedTask {
    fn eq(&self, other: &Self) -> bool {
        self.0.manual_order == other.0.manual_order
    }
}

impl Eq for OrderedTask {}

impl PartialOrd for OrderedTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .0
            .manual_order
            .partial_cmp(&self.0.manual_order)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

// Helper to clone a connection for the sort method
impl Clone for Connection {
    fn clone(&self) -> Self {
        Self::open_default().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::Schema;

    fn setup_repo() -> TaskRepository {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();
        TaskRepository { conn }
    }

    #[test]
    fn test_create_get_task() {
        let mut repo = setup_repo();

        let task = repo
            .create_task("Test Task".to_string(), None, None, 10.0)
            .unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Pending);

        let fetched = repo.get_task(task.id).unwrap();
        assert_eq!(fetched.id, task.id);
    }

    #[test]
    fn test_start_task() {
        let mut repo = setup_repo();

        let task = repo
            .create_task("Test Task".to_string(), None, None, 10.0)
            .unwrap();
        let started = repo.start_task(task.id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
        assert!(started.started_at.is_some());
    }

    #[test]
    fn test_start_task_idempotent() {
        let mut repo = setup_repo();

        let task = repo
            .create_task("Test Task".to_string(), None, None, 10.0)
            .unwrap();
        repo.start_task(task.id).unwrap();
        let started = repo.start_task(task.id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_complete_task_requires_dod() {
        let mut repo = setup_repo();

        let task = repo
            .create_task("Test Task".to_string(), None, None, 10.0)
            .unwrap();
        repo.start_task(task.id).unwrap();

        let result = repo.complete_task();
        assert!(matches!(result, Err(Error::NoDod(1))));
    }

    #[test]
    fn test_complete_task() {
        let mut repo = setup_repo();

        let task = repo
            .create_task(
                "Test Task".to_string(),
                None,
                Some("Done".to_string()),
                10.0,
            )
            .unwrap();
        repo.start_task(task.id).unwrap();

        let completed = repo.complete_task().unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_set_get_target() {
        let mut repo = setup_repo();

        let task = repo
            .create_task("Target Task".to_string(), None, None, 10.0)
            .unwrap();
        repo.set_target(task.id).unwrap();

        assert_eq!(repo.get_target().unwrap(), Some(task.id));
    }

    #[test]
    fn test_get_next_task() {
        let mut repo = setup_repo();

        // Create tasks: 1 depends on nothing, 2 depends on 1
        let task1 = repo
            .create_task("Task 1".to_string(), None, Some("Done".to_string()), 10.0)
            .unwrap();
        let task2 = repo
            .create_task("Task 2".to_string(), None, None, 20.0)
            .unwrap();
        dependency::add_dependency(&mut repo.conn, 2, 1).unwrap();

        repo.set_target(2).unwrap();

        // Complete task 1 first
        repo.start_task(task1.id).unwrap();
        repo.complete_task().unwrap();

        // Next should be task 2
        let next = repo.get_next_task().unwrap();
        assert_eq!(next.id, 2);
    }
}
