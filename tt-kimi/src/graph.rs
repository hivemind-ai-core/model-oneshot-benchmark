use crate::error::{Result, TaskError};
use crate::models::{OrderConflict, Status, Task};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// A task with its ordering information for the priority queue
#[derive(Debug, Clone)]
struct HeapTask {
    manual_order: f64,
    task_id: i64,
}

impl Eq for HeapTask {}

impl PartialEq for HeapTask {
    fn eq(&self, other: &Self) -> bool {
        self.manual_order == other.manual_order && self.task_id == other.task_id
    }
}

impl Ord for HeapTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap (lower manual_order = higher priority)
        other
            .manual_order
            .partial_cmp(&self.manual_order)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.task_id.cmp(&self.task_id))
    }
}

impl PartialOrd for HeapTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Result of a topological sort
#[derive(Debug, Clone)]
pub struct TopologicalSort {
    pub ordered_tasks: Vec<Task>,
    pub order_conflicts: Vec<OrderConflict>,
}

/// Perform Kahn's algorithm with priority queue for topological sort
pub fn topological_sort(tasks_with_deps: Vec<(Task, Vec<i64>)>) -> Result<TopologicalSort> {
    let n = tasks_with_deps.len();
    if n == 0 {
        return Ok(TopologicalSort {
            ordered_tasks: vec![],
            order_conflicts: vec![],
        });
    }

    // Build task map
    let task_map: HashMap<i64, Task> = tasks_with_deps
        .iter()
        .map(|(t, _)| (t.id, t.clone()))
        .collect();

    // Build in-degree map and adjacency list
    let mut in_degree: HashMap<i64, usize> = HashMap::new();
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();

    for (task, deps) in &tasks_with_deps {
        in_degree.entry(task.id).or_insert(0);
        for &dep_id in deps {
            // Only count dependencies that are in our task set
            // Dependencies outside the set (e.g., completed tasks) are considered satisfied
            if task_map.contains_key(&dep_id) {
                adjacency.entry(dep_id).or_default().push(task.id);
                *in_degree.entry(task.id).or_insert(0) += 1;
            }
        }
    }

    // Seed min-heap with in-degree 0 tasks
    let mut heap: BinaryHeap<HeapTask> = BinaryHeap::new();
    for (task_id, degree) in &in_degree {
        if *degree == 0 {
            if let Some(task) = task_map.get(task_id) {
                heap.push(HeapTask {
                    manual_order: task.manual_order,
                    task_id: *task_id,
                });
            }
        }
    }

    // Kahn's algorithm
    let mut result: Vec<Task> = Vec::with_capacity(n);

    while let Some(heap_task) = heap.pop() {
        let task = task_map.get(&heap_task.task_id).unwrap().clone();
        result.push(task);

        if let Some(dependents) = adjacency.get(&heap_task.task_id) {
            for &dependent_id in dependents {
                if let Some(degree) = in_degree.get_mut(&dependent_id) {
                    *degree -= 1;
                    if *degree == 0 {
                        if let Some(dep_task) = task_map.get(&dependent_id) {
                            heap.push(HeapTask {
                                manual_order: dep_task.manual_order,
                                task_id: dependent_id,
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for order conflicts
    let order_conflicts = detect_order_conflicts(&result, &tasks_with_deps);

    Ok(TopologicalSort {
        ordered_tasks: result,
        order_conflicts,
    })
}

/// Detect order conflicts where a task has lower manual_order than its dependency
fn detect_order_conflicts(
    sorted_tasks: &[Task],
    tasks_with_deps: &[(Task, Vec<i64>)],
) -> Vec<OrderConflict> {
    let mut conflicts = Vec::new();
    let order_map: HashMap<i64, f64> = sorted_tasks
        .iter()
        .map(|t| (t.id, t.manual_order))
        .collect();
    let deps_map: HashMap<i64, Vec<i64>> = tasks_with_deps
        .iter()
        .map(|(t, deps)| (t.id, deps.clone()))
        .collect();

    for task in sorted_tasks {
        if let Some(deps) = deps_map.get(&task.id) {
            for &dep_id in deps {
                if let Some(&dep_order) = order_map.get(&dep_id) {
                    if task.manual_order < dep_order {
                        conflicts.push(OrderConflict {
                            task_id: task.id,
                            task_order: task.manual_order,
                            dep_id,
                            dep_order,
                        });
                    }
                }
            }
        }
    }

    conflicts
}

/// Check if adding a dependency would create a cycle using DFS
/// from: the task that will depend on 'to'
/// to: the task that 'from' will depend on
/// Returns the cycle path if a cycle would be created
pub fn would_create_cycle(all_deps: &[(i64, i64)], from: i64, to: i64) -> Option<Vec<i64>> {
    // Build adjacency list where edge A -> B means "A depends on B"
    // So if from depends on to, we have edge: from -> to
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();

    // Add existing dependencies: task_id depends_on depends_on
    // Edge: task_id -> depends_on
    for (task_id, depends_on) in all_deps {
        adjacency.entry(*task_id).or_default().push(*depends_on);
    }

    // Temporarily add the new dependency: from depends_on to
    // Edge: from -> to
    adjacency.entry(from).or_default().push(to);

    // Check if there's now a path from 'to' back to 'from'
    // If yes, we have a cycle: from -> to -> ... -> from
    let mut visited: HashSet<i64> = HashSet::new();
    let mut path: Vec<i64> = vec![];

    if dfs_find_path(to, from, &adjacency, &mut visited, &mut path) {
        // Cycle found: from -> to -> path -> from
        let mut cycle = vec![from, to];
        cycle.extend(path);
        Some(cycle)
    } else {
        None
    }
}

/// DFS to find a path from 'current' to 'target'
fn dfs_find_path(
    current: i64,
    target: i64,
    adjacency: &HashMap<i64, Vec<i64>>,
    visited: &mut HashSet<i64>,
    path: &mut Vec<i64>,
) -> bool {
    if current == target {
        return true;
    }

    if visited.contains(&current) {
        return false;
    }

    visited.insert(current);

    if let Some(neighbors) = adjacency.get(&current) {
        for &neighbor in neighbors {
            path.push(neighbor);
            if dfs_find_path(neighbor, target, adjacency, visited, path) {
                return true;
            }
            path.pop();
        }
    }

    visited.remove(&current);
    false
}

/// Calculate a midpoint between two orders
/// Returns Err if float precision is exhausted
pub fn calculate_midpoint(a: f64, b: f64) -> Result<f64> {
    let mid = (a + b) / 2.0;
    if mid == a || mid == b {
        return Err(TaskError::FloatPrecisionExhausted);
    }
    Ok(mid)
}

/// Calculate order for inserting after a task
pub fn calculate_order_after(a: f64) -> f64 {
    a + 10.0
}

/// Calculate order for inserting before a task
pub fn calculate_order_before(b: f64) -> f64 {
    b - 10.0
}

/// Get initial order for a new task
pub fn get_initial_order(max_order: f64) -> f64 {
    if max_order == 0.0 {
        10.0
    } else {
        max_order + 10.0
    }
}

/// Generate reindexed orders
pub fn reindex_orders(tasks: &[Task]) -> Vec<(i64, f64)> {
    tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let new_order = (i as f64 + 1.0) * 10.0;
            (task.id, new_order)
        })
        .collect()
}

/// Check if a task can be started (all dependencies completed)
pub fn can_start_task(_task_id: i64, deps: &[(i64, Status)]) -> Option<Vec<i64>> {
    let unmet: Vec<i64> = deps
        .iter()
        .filter(|(_, status)| *status != Status::Completed)
        .map(|(id, _)| *id)
        .collect();

    if unmet.is_empty() { None } else { Some(unmet) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_task(id: i64, manual_order: f64) -> Task {
        Task {
            id,
            title: format!("Task {id}"),
            description: None,
            dod: None,
            status: Status::Pending,
            manual_order,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            last_touched_at: Utc::now(),
        }
    }

    #[test]
    fn test_topological_sort_linear_chain() {
        // A -> B -> C (A must come first)
        let a = create_task(1, 10.0);
        let b = create_task(2, 20.0);
        let c = create_task(3, 30.0);

        let tasks = vec![
            (a.clone(), vec![]),
            (b.clone(), vec![1]),
            (c.clone(), vec![2]),
        ];

        let result = topological_sort(tasks).unwrap();
        assert_eq!(result.ordered_tasks.len(), 3);
        assert_eq!(result.ordered_tasks[0].id, 1);
        assert_eq!(result.ordered_tasks[1].id, 2);
        assert_eq!(result.ordered_tasks[2].id, 3);
    }

    #[test]
    fn test_topological_sort_diamond() {
        // Diamond: A -> B, A -> C, B -> D, C -> D
        let a = create_task(1, 10.0);
        let b = create_task(2, 20.0);
        let c = create_task(3, 15.0); // Lower order than B
        let d = create_task(4, 40.0);

        let tasks = vec![
            (a.clone(), vec![]),
            (b.clone(), vec![1]),
            (c.clone(), vec![1]),
            (d.clone(), vec![2, 3]),
        ];

        let result = topological_sort(tasks).unwrap();
        assert_eq!(result.ordered_tasks.len(), 4);
        assert_eq!(result.ordered_tasks[0].id, 1); // A first
        assert_eq!(result.ordered_tasks[3].id, 4); // D last
        // B and C come after A, before D
        assert!(result.ordered_tasks[1].id == 2 || result.ordered_tasks[1].id == 3);
        assert!(result.ordered_tasks[2].id == 2 || result.ordered_tasks[2].id == 3);
    }

    #[test]
    fn test_cycle_detection() {
        // A -> B -> C -> A (cycle)
        let deps = vec![(2, 1), (3, 2)]; // B depends on A, C depends on B

        // Try adding C -> A (would create cycle: 1 -> 3 -> ... -> 1)
        let cycle = would_create_cycle(&deps, 1, 3);
        assert!(cycle.is_some());
        let path = cycle.unwrap();
        // Cycle path should start and end with 'from' node (1)
        assert_eq!(path[0], 1);
        assert_eq!(path[path.len() - 1], 1);
        // Path should contain all 3 nodes
        assert!(path.contains(&1));
        assert!(path.contains(&2));
        assert!(path.contains(&3));
        // Path length should be 4 (start -> ... -> end which is same as start)
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn test_no_cycle() {
        // A -> B -> C (no cycle)
        let deps = vec![(2, 1), (3, 2)];

        // Adding C -> D is fine
        let cycle = would_create_cycle(&deps, 4, 3);
        assert!(cycle.is_none());
    }

    #[test]
    fn test_midpoint_calculation() {
        assert_eq!(calculate_midpoint(1.0, 2.0).unwrap(), 1.5);
        assert_eq!(calculate_midpoint(10.0, 20.0).unwrap(), 15.0);
    }

    #[test]
    fn test_order_after() {
        assert_eq!(calculate_order_after(10.0), 20.0);
        assert_eq!(calculate_order_after(25.0), 35.0);
    }

    #[test]
    fn test_order_before() {
        assert_eq!(calculate_order_before(20.0), 10.0);
        assert_eq!(calculate_order_before(35.0), 25.0);
    }

    #[test]
    fn test_reindex() {
        let tasks = vec![
            create_task(5, 15.0),
            create_task(1, 10.0),
            create_task(3, 12.5),
        ];

        let reindexed = reindex_orders(&tasks);
        assert_eq!(reindexed.len(), 3);
        // Should be 10.0, 20.0, 30.0 in order of tasks array
        assert_eq!(reindexed[0], (5, 10.0));
        assert_eq!(reindexed[1], (1, 20.0));
        assert_eq!(reindexed[2], (3, 30.0));
    }

    #[test]
    fn test_can_start_task() {
        let deps = vec![
            (1, Status::Completed),
            (2, Status::Completed),
            (3, Status::Pending),
        ];

        let unmet = can_start_task(10, &deps);
        assert!(unmet.is_some());
        assert_eq!(unmet.unwrap(), vec![3]);

        let deps_completed = vec![(1, Status::Completed), (2, Status::Completed)];
        assert!(can_start_task(10, &deps_completed).is_none());
    }
}
