//! Topological sorting using Kahn's algorithm.

use crate::core::Task;
use crate::Result;
use std::collections::{BinaryHeap, HashMap};

/// Sort tasks topologically with manual_order as tiebreaker.
///
/// Uses Kahn's algorithm with a min-heap (ordered by manual_order).
/// Returns tasks in execution order where all dependencies come before dependents.
pub fn topological_sort(tasks: Vec<Task>) -> Result<Vec<Task>> {
    // Build adjacency list and in-degree map
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut in_degree: HashMap<i64, i32> = HashMap::new();
    let mut task_map: HashMap<i64, Task> = HashMap::new();

    for task in &tasks {
        task_map.insert(task.id, task.clone());
        adjacency.entry(task.id).or_default();
        in_degree.entry(task.id).or_insert(0);
    }

    // We need to get dependencies from somewhere
    // For now, this is a placeholder - we'll need to pass dependencies in
    // or have a way to query them

    // Use a min-heap based on manual_order
    let mut heap: BinaryHeap<OrderedTask> = BinaryHeap::new();

    // Find all tasks with in-degree 0
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
        result.push(ordered_task.0.clone());

        // Decrement in-degree of dependents
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

/// Wrapper for min-heap ordering by manual_order.
///
/// BinaryHeap is a max-heap, so we reverse the ordering.
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
        // Reverse for min-heap behavior
        other
            .0
            .manual_order
            .partial_cmp(&self.0.manual_order)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TaskStatus;

    fn make_task(id: i64, order: f64) -> Task {
        Task {
            id,
            title: format!("Task {id}"),
            description: None,
            dod: None,
            status: TaskStatus::Pending,
            manual_order: order,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            last_touched_at: String::new(),
        }
    }

    #[test]
    fn test_ordered_task_cmp() {
        let t1 = OrderedTask(make_task(1, 10.0));
        let t2 = OrderedTask(make_task(2, 20.0));
        let t3 = OrderedTask(make_task(3, 10.0));

        // Lower manual_order should be "greater" (comes first)
        assert!(t1 > t2);
        assert_eq!(t1, t3);
    }

    #[test]
    fn test_topological_sort_no_dependencies() {
        let tasks = vec![make_task(1, 30.0), make_task(2, 10.0), make_task(3, 20.0)];

        let result = topological_sort(tasks).unwrap();
        assert_eq!(result.len(), 3);
        // Should be ordered by manual_order: 2, 3, 1
        assert_eq!(result[0].id, 2);
        assert_eq!(result[1].id, 3);
        assert_eq!(result[2].id, 1);
    }
}
