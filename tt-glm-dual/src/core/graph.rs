//! Graph engine for the tt task tracker.
//!
//! Handles topological sorting, cycle detection, and target subgraph operations.

use crate::core::db::{Db, Task};
use crate::core::error::{Result, TTError};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Wrapper for Task to use in min-heap based on manual_order.
#[derive(Debug, Clone, PartialEq)]
struct MinHeapTask {
    task_id: i64,
    manual_order: f64,
}

impl Eq for MinHeapTask {}

impl Ord for MinHeapTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap behavior
        other
            .manual_order
            .partial_cmp(&self.manual_order)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.task_id.cmp(&self.task_id))
    }
}

impl PartialOrd for MinHeapTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Perform topological sort using Kahn's algorithm with manual_order tiebreaking.
pub fn topological_sort(db: &Db, task_ids: &[i64]) -> Result<Vec<TaskInOrder>> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch all tasks and build maps
    let mut tasks_by_id: HashMap<i64, Task> = HashMap::new();
    let mut in_degree: HashMap<i64, usize> = HashMap::new();
    let mut adj_list: HashMap<i64, Vec<i64>> = HashMap::new();

    // Initialize in-degree for all tasks
    for &id in task_ids {
        in_degree.insert(id, 0);
        adj_list.insert(id, Vec::new());
    }

    // Build the graph
    for &id in task_ids {
        let task = db.get_task(id)?;
        tasks_by_id.insert(id, task);

        let deps = db.get_dependencies(id)?;
        for &dep_id in &deps {
            if task_ids.contains(&dep_id) {
                *in_degree.entry(id).or_insert(0) += 1;
                adj_list.entry(dep_id).or_default().push(id);
            }
        }
    }

    // Seed the heap with tasks that have in-degree 0
    let mut heap: BinaryHeap<MinHeapTask> = BinaryHeap::new();
    for (&id, degree) in &in_degree {
        if *degree == 0 {
            if let Some(task) = tasks_by_id.get(&id) {
                heap.push(MinHeapTask {
                    task_id: id,
                    manual_order: task.manual_order,
                });
            }
        }
    }

    let mut result = Vec::new();
    let mut completed: HashSet<i64> = HashSet::new();

    while let Some(min_task) = heap.pop() {
        let id = min_task.task_id;
        let task = tasks_by_id.get(&id).unwrap().clone();

        // Check if all dependencies are completed
        let deps = db.get_dependencies(id)?;
        let all_deps_completed = deps.iter().all(|&dep_id| {
            if let Ok(dep_task) = db.get_task(dep_id) {
                dep_task.status == "completed"
            } else {
                false
            }
        });

        result.push(TaskInOrder {
            task,
            all_deps_completed,
        });

        completed.insert(id);

        // Decrement in-degree of dependents
        if let Some(dependents) = adj_list.get(&id) {
            for &dep_id in dependents {
                if let Some(degree) = in_degree.get_mut(&dep_id) {
                    *degree -= 1;
                    if *degree == 0 {
                        if let Some(task) = tasks_by_id.get(&dep_id) {
                            heap.push(MinHeapTask {
                                task_id: dep_id,
                                manual_order: task.manual_order,
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for cycles
    if result.len() != task_ids.len() {
        return Err(TTError::InvalidStatus(
            "Cycle detected in task graph".to_string(),
        ));
    }

    Ok(result)
}

/// Check for cycles when adding a dependency edge.
pub fn check_cycle(db: &Db, from: i64, to: i64) -> Result<()> {
    if db.path_exists(from, to)? {
        let path = db.get_path(from, to)?;
        return Err(TTError::CycleDetected(from, to, path));
    }
    Ok(())
}

/// Calculate midpoint order between two tasks.
pub fn calculate_midpoint(db: &Db, after_id: Option<i64>, before_id: Option<i64>) -> Result<f64> {
    match (after_id, before_id) {
        (Some(after), Some(before)) => {
            let after_task = db.get_task(after)?;
            let before_task = db.get_task(before)?;
            let mid = (after_task.manual_order + before_task.manual_order) / 2.0;

            // Check for float precision exhaustion
            if mid == after_task.manual_order || mid == before_task.manual_order {
                return Err(TTError::FloatPrecisionExhausted);
            }

            Ok(mid)
        }
        (Some(after), None) => {
            let after_task = db.get_task(after)?;
            Ok(after_task.manual_order + 10.0)
        }
        (None, Some(before)) => {
            let before_task = db.get_task(before)?;
            let result = before_task.manual_order - 10.0;
            if result == before_task.manual_order {
                return Err(TTError::FloatPrecisionExhausted);
            }
            Ok(result)
        }
        (None, None) => {
            let max = db.get_max_manual_order()?;
            Ok(max + 10.0)
        }
    }
}

/// Check for order conflicts and return warnings.
pub fn check_order_conflicts(db: &Db, task_ids: &[i64]) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    for &id in task_ids {
        let task = db.get_task(id)?;
        let deps = db.get_dependencies(id)?;

        for &dep_id in &deps {
            if task_ids.contains(&dep_id) {
                let dep_task = db.get_task(dep_id)?;
                if task.manual_order < dep_task.manual_order {
                    warnings.push(format!(
                        "Warning: #{} (order {}) depends on #{} (order {}) which has higher manual_order",
                        id, task.manual_order, dep_id, dep_task.manual_order
                    ));
                }
            }
        }
    }

    Ok(warnings)
}

/// Task in topological order with completion status.
#[derive(Debug, Clone)]
pub struct TaskInOrder {
    pub task: Task,
    pub all_deps_completed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_topological_sort_linear() {
        let temp = NamedTempFile::new().unwrap();
        let db = Db::open(temp.path()).unwrap();
        db.init_schema().unwrap();

        let id1 = db.create_task("Task 1", None, None, 10.0).unwrap();
        let id2 = db.create_task("Task 2", None, None, 20.0).unwrap();
        let id3 = db.create_task("Task 3", None, None, 30.0).unwrap();

        db.add_dependency(id2, id1).unwrap();
        db.add_dependency(id3, id2).unwrap();

        let sorted = topological_sort(&db, &[id1, id2, id3]).unwrap();
        assert_eq!(sorted[0].task.id, id1);
        assert_eq!(sorted[1].task.id, id2);
        assert_eq!(sorted[2].task.id, id3);
    }

    #[test]
    fn test_midpoint_calculation() {
        let temp = NamedTempFile::new().unwrap();
        let db = Db::open(temp.path()).unwrap();
        db.init_schema().unwrap();

        let id1 = db.create_task("Task 1", None, None, 10.0).unwrap();
        let id2 = db.create_task("Task 2", None, None, 20.0).unwrap();

        let mid = calculate_midpoint(&db, Some(id1), Some(id2)).unwrap();
        assert_eq!(mid, 15.0);
    }
}
