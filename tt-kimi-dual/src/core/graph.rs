use crate::core::error::{TTError, TTResult};
use crate::core::models::{OrderConflict, Task};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Wrapper for min-heap behavior (BinaryHeap is max-heap by default)
/// Uses task ID as tiebreaker to ensure consistent ordering
#[derive(Debug, Clone)]
struct TaskNode {
    order: f64,
    task_id: i64,
    task: Task,
}

impl PartialEq for TaskNode {
    fn eq(&self, other: &Self) -> bool {
        self.order == other.order && self.task_id == other.task_id
    }
}

impl Eq for TaskNode {}

impl PartialOrd for TaskNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by order (lower first = higher priority)
        // Handle NaN by treating it as greater than any number
        let order_cmp = match (self.order.is_nan(), other.order.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => self
                .order
                .partial_cmp(&other.order)
                .unwrap_or(Ordering::Equal),
        };

        // Reverse for min-heap
        match order_cmp.reverse() {
            Ordering::Equal => self.task_id.cmp(&other.task_id),
            other => other,
        }
    }
}

/// Topological sort using Kahn's algorithm with priority queue (min-heap by manual_order)
pub fn topological_sort(
    tasks: &[Task],
    dependencies: &[(i64, i64)],
) -> (Vec<Task>, Vec<OrderConflict>) {
    let task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();

    // Build adjacency list and in-degree map
    let mut in_degree: HashMap<i64, usize> = HashMap::new();
    let mut adj: HashMap<i64, Vec<i64>> = HashMap::new();
    let task_map: HashMap<i64, &Task> = tasks.iter().map(|t| (t.id, t)).collect();

    // Initialize all tasks with in-degree 0
    for task in tasks {
        in_degree.entry(task.id).or_insert(0);
        adj.entry(task.id).or_default();
    }

    // Build graph from dependencies
    for (task_id, depends_on) in dependencies {
        // Only include dependencies where both tasks are in our set
        if task_ids.contains(task_id) && task_ids.contains(depends_on) {
            adj.entry(*depends_on).or_default().push(*task_id);
            *in_degree.entry(*task_id).or_insert(0) += 1;
        }
    }

    // Seed min-heap with tasks that have in-degree 0
    let mut heap: BinaryHeap<TaskNode> = BinaryHeap::new();
    for task in tasks {
        if in_degree.get(&task.id).copied().unwrap_or(0) == 0 {
            heap.push(TaskNode {
                order: task.manual_order,
                task_id: task.id,
                task: task.clone(),
            });
        }
    }

    let mut result = Vec::new();

    // Kahn's algorithm
    while let Some(node) = heap.pop() {
        let task = node.task;
        result.push(task.clone());

        if let Some(dependents) = adj.get(&task.id) {
            for &dependent_id in dependents {
                if let Some(deg) = in_degree.get_mut(&dependent_id) {
                    *deg -= 1;
                    if *deg == 0 {
                        if let Some(&dep_task) = task_map.get(&dependent_id) {
                            heap.push(TaskNode {
                                order: dep_task.manual_order,
                                task_id: dep_task.id,
                                task: dep_task.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Check for order conflicts
    let conflicts = detect_order_conflicts(&result, dependencies);

    (result, conflicts)
}

/// Detect order conflicts where a task has lower manual_order than its dependency
fn detect_order_conflicts(tasks: &[Task], dependencies: &[(i64, i64)]) -> Vec<OrderConflict> {
    let task_map: HashMap<i64, &Task> = tasks.iter().map(|t| (t.id, t)).collect();
    let mut conflicts = Vec::new();

    for (task_id, depends_on) in dependencies {
        if let (Some(&task), Some(&dep_task)) = (task_map.get(task_id), task_map.get(depends_on)) {
            if task.manual_order < dep_task.manual_order {
                conflicts.push(OrderConflict {
                    task_id: task.id,
                    task_order: task.manual_order,
                    dep_id: dep_task.id,
                    dep_order: dep_task.manual_order,
                });
            }
        }
    }

    conflicts
}

/// Detect cycles using DFS. Returns the cycle path if found.
/// When adding a dependency `from depends on to`, we need to check if `to` is already
/// reachable from `from` through existing dependencies. If so, adding this edge would create a cycle.
pub fn detect_cycle(from: i64, to: i64, existing_deps: &[(i64, i64)]) -> Option<Vec<i64>> {
    // Build adjacency list: for each task, list what it depends on (prerequisites)
    let mut adj: HashMap<i64, Vec<i64>> = HashMap::new();

    // Add existing dependencies: task_id depends on depends_on
    for (task_id, depends_on) in existing_deps {
        adj.entry(*task_id).or_default().push(*depends_on);
    }

    // Check: can we reach 'from' by starting from 'to' through the dependency graph?
    // If yes, then adding "from depends on to" would create a cycle.
    let mut visited = HashSet::new();
    let mut path = Vec::new();

    if dfs_find_cycle(to, from, &adj, &mut visited, &mut path) {
        // Cycle found: path goes from 'to' to 'from', add 'to' at end to complete cycle
        let mut cycle = path.clone();
        cycle.push(to);
        Some(cycle)
    } else {
        None
    }
}

fn dfs_find_cycle(
    current: i64,
    target: i64,
    adj: &HashMap<i64, Vec<i64>>,
    visited: &mut HashSet<i64>,
    path: &mut Vec<i64>,
) -> bool {
    if visited.contains(&current) {
        return false;
    }

    // Add current to path before checking if it's the target
    // This ensures the path includes nodes from start to target
    visited.insert(current);
    path.push(current);

    if current == target {
        return true;
    }

    if let Some(prereqs) = adj.get(&current) {
        for &prereq in prereqs {
            if dfs_find_cycle(prereq, target, adj, visited, path) {
                return true;
            }
        }
    }

    path.pop();
    false
}

/// Calculate midpoint between two order values
/// Returns Err if float precision is exhausted
pub fn calculate_midpoint(a: f64, b: f64) -> TTResult<f64> {
    let mid = (a + b) / 2.0;
    if mid == a || mid == b {
        return Err(TTError::FloatPrecisionExhausted);
    }
    Ok(mid)
}

/// Calculate new order value for inserting after a task
pub fn order_after(order: f64) -> f64 {
    order + 10.0
}

/// Calculate new order value for inserting before a task
pub fn order_before(order: f64) -> f64 {
    // Ensure we don't go negative
    (order - 10.0).max(0.0)
}

/// Calculate default order for new task (MAX + 10.0)
pub fn default_order(max_order: f64) -> f64 {
    if max_order == 0.0 {
        10.0
    } else {
        max_order + 10.0
    }
}

/// Build reindexed orders (10.0, 20.0, 30.0, ...) preserving current sorted order
pub fn reindex_orders(tasks: &[Task]) -> Vec<(i64, f64)> {
    tasks
        .iter()
        .enumerate()
        .map(|(i, task)| (task.id, ((i + 1) as f64) * 10.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::TaskStatus;
    use chrono::Utc;

    fn create_test_task(id: i64, title: &str, order: f64) -> Task {
        Task {
            id,
            title: title.to_string(),
            description: None,
            dod: None,
            status: TaskStatus::Pending,
            manual_order: order,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            last_touched_at: Utc::now(),
        }
    }

    #[test]
    fn test_topological_sort_linear() {
        let a = create_test_task(1, "A", 10.0);
        let b = create_test_task(2, "B", 20.0);
        let c = create_test_task(3, "C", 30.0);

        let tasks = vec![a.clone(), b.clone(), c.clone()];
        // A -> B -> C (B depends on A, C depends on B)
        let deps = vec![(2, 1), (3, 2)];

        let (sorted, conflicts) = topological_sort(&tasks, &deps);
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].id, 1); // A first
        assert_eq!(sorted[1].id, 2); // B second
        assert_eq!(sorted[2].id, 3); // C third
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_topological_sort_diamond() {
        // Diamond: A -> B, A -> C, B -> D, C -> D
        let a = create_test_task(1, "A", 10.0);
        let b = create_test_task(2, "B", 20.0);
        let c = create_test_task(3, "C", 30.0);
        let d = create_test_task(4, "D", 40.0);

        let tasks = vec![a.clone(), b.clone(), c.clone(), d.clone()];
        let deps = vec![(2, 1), (3, 1), (4, 2), (4, 3)];

        let (sorted, _) = topological_sort(&tasks, &deps);
        assert_eq!(sorted.len(), 4);
        assert_eq!(sorted[0].id, 1); // A first
        assert_eq!(sorted[3].id, 4); // D last
                                     // B and C order depends on manual_order
        assert!(sorted[1].id == 2 || sorted[1].id == 3);
        assert!(sorted[2].id == 2 || sorted[2].id == 3);
    }

    #[test]
    fn test_topological_sort_manual_order_tiebreak() {
        // Two independent tasks, lower manual_order should come first
        let a = create_test_task(1, "A", 5.0);
        let b = create_test_task(2, "B", 3.0);

        let tasks = vec![a.clone(), b.clone()];
        let deps: Vec<(i64, i64)> = vec![];

        let (sorted, _) = topological_sort(&tasks, &deps);
        assert_eq!(sorted[0].id, 2); // B has lower order
        assert_eq!(sorted[1].id, 1); // A has higher order
    }

    #[test]
    fn test_cycle_detection() {
        // A -> B -> C -> A
        let existing_deps = vec![(2, 1), (3, 2)]; // B depends on A, C depends on B

        // Try to add C -> A (1 depends on 3)
        let cycle = detect_cycle(1, 3, &existing_deps);
        assert!(cycle.is_some());
        let path = cycle.unwrap();
        assert!(path.contains(&1));
        assert!(path.contains(&2));
        assert!(path.contains(&3));
    }

    #[test]
    fn test_no_cycle() {
        let existing_deps = vec![(2, 1), (3, 2)];

        // Try to add D -> C (4 depends on 3) - no cycle
        let cycle = detect_cycle(4, 3, &existing_deps);
        assert!(cycle.is_none());
    }

    #[test]
    fn test_midpoint() {
        assert_eq!(calculate_midpoint(1.0, 2.0).unwrap(), 1.5);
        assert_eq!(calculate_midpoint(10.0, 20.0).unwrap(), 15.0);
    }

    #[test]
    fn test_order_conflict_detection() {
        // Task 1 depends on Task 2, but Task 1 has lower manual_order
        let t1 = create_test_task(1, "Task 1", 10.0);
        let t2 = create_test_task(2, "Task 2", 20.0);

        let tasks = vec![t1, t2];
        let deps = vec![(1, 2)]; // Task 1 depends on Task 2

        let (sorted, conflicts) = topological_sort(&tasks, &deps);
        assert_eq!(sorted.len(), 2);
        // Topological order should still be correct: Task 2 before Task 1
        assert_eq!(sorted[0].id, 2);
        assert_eq!(sorted[1].id, 1);

        // But there should be a conflict warning
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].task_id, 1);
        assert_eq!(conflicts[0].dep_id, 2);
    }

    #[test]
    fn test_reindex() {
        let a = create_test_task(1, "A", 15.5);
        let b = create_test_task(2, "B", 3.2);
        let c = create_test_task(3, "C", 99.9);

        let tasks = vec![b, a, c]; // Not in order by manual_order
        let new_orders = reindex_orders(&tasks);

        assert_eq!(new_orders.len(), 3);
        // IDs preserved, orders are now clean multiples of 10
        let order_map: HashMap<i64, f64> = new_orders.into_iter().collect();
        assert_eq!(order_map[&1], 20.0); // b was first
        assert_eq!(order_map[&2], 10.0); // a was second
        assert_eq!(order_map[&3], 30.0); // c was third
    }
}
