//! Cycle detection in dependency graph.

use crate::Result;
use std::collections::{HashMap, HashSet};

/// A path representing a cycle in the dependency graph.
#[derive(Debug, Clone)]
pub struct CyclePath {
    pub path: Vec<i64>,
}

impl CyclePath {
    /// Create a new cycle path.
    pub fn new(path: Vec<i64>) -> Self {
        Self { path }
    }

    /// Format the cycle as a string.
    pub fn format(&self) -> String {
        self.path
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(" → ")
    }
}

/// Detect if adding an edge from `from_id` to `to_id` would create a cycle.
///
/// We want to add: `from_id depends on to_id` (from_id -> to_id)
/// A cycle would be created if there's already a path from `to_id` back to `from_id`.
///
/// The `dependencies` HashMap maps: task_id -> Vec of dependencies (what this task depends on)
/// So if deps[2] = [1], then task 2 depends on task 1 (2 -> 1)
pub fn detect_cycle(
    from_id: i64,
    to_id: i64,
    dependencies: &HashMap<i64, Vec<i64>>,
) -> Result<Option<CyclePath>> {
    // We want to add: from_id -> to_id
    // Check if there's already a path from to_id back to from_id

    let mut visited = HashSet::new();

    if let Some(path) = find_path(to_id, from_id, dependencies, &mut visited) {
        // We found a path: to_id -> ... -> from_id
        // Adding from_id -> to_id would create: to_id -> ... -> from_id -> to_id
        let mut cycle = path;
        cycle.push(to_id); // Close the cycle
        return Ok(Some(CyclePath::new(cycle)));
    }

    Ok(None)
}

/// DFS to find a path from start to target through the dependency graph.
///
/// Returns Some(path) if found, where path is [start, ..., target]
/// Returns None if no path exists.
///
/// The graph is: task -> dependencies (what this task depends on)
/// So deps[2] = [1] means task 2 depends on task 1
fn find_path(
    current: i64,
    target: i64,
    dependencies: &HashMap<i64, Vec<i64>>,
    visited: &mut HashSet<i64>,
) -> Option<Vec<i64>> {
    if visited.contains(&current) {
        return None;
    }

    visited.insert(current);

    if current == target {
        return Some(vec![current]);
    }

    // Look at what current depends on
    if let Some(deps) = dependencies.get(&current) {
        for &dep in deps {
            if let Some(mut path) = find_path(dep, target, dependencies, visited) {
                // Prepend current to the path
                path.insert(0, current);
                return Some(path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cycle_none() {
        // Current dependencies:
        // - Task 2 depends on 1 (2 -> 1)
        // - Task 3 depends on 2 (3 -> 2)
        // Adding "task 4 depends on 1" (4 -> 1) should not create a cycle
        // because there's no path from 1 to 4
        let mut deps = HashMap::new();
        deps.insert(2, vec![1]); // 2 -> 1
        deps.insert(3, vec![2]); // 3 -> 2

        let result = detect_cycle(4, 1, &deps).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_cycle_found() {
        // Current dependencies:
        // - Task 1 depends on 2 (1 -> 2)
        // - Task 2 depends on 3 (2 -> 3)
        // Adding "task 3 depends on 1" (3 -> 1) would create: 1 -> 2 -> 3 -> 1
        let mut deps = HashMap::new();
        deps.insert(1, vec![2]); // 1 -> 2
        deps.insert(2, vec![3]); // 2 -> 3

        // Check if adding "3 depends on 1" creates a cycle
        // Does 1 have a path to 3? Yes: 1 -> 2 -> 3
        let result = detect_cycle(3, 1, &deps).unwrap();
        assert!(result.is_some());

        let cycle = result.unwrap();
        // Cycle: 1 -> 2 -> 3 -> 1
        assert_eq!(cycle.path, vec![1, 2, 3, 1]);
    }

    #[test]
    fn test_cycle_path_format() {
        let path = CyclePath::new(vec![1, 2, 3, 1]);
        assert_eq!(path.format(), "#1 → #2 → #3 → #1");
    }

    #[test]
    fn test_detect_cycle_immediate() {
        // Current: 1 depends on 2 (1 -> 2)
        // Adding "2 depends on 1" (2 -> 1) would create: 1 -> 2 -> 1
        let mut deps = HashMap::new();
        deps.insert(1, vec![2]); // 1 -> 2

        // Check if adding "2 depends on 1" creates a cycle
        // Does 1 have a path to 2? Yes, directly!
        let result = detect_cycle(2, 1, &deps).unwrap();
        assert!(result.is_some());
        // Cycle: 1 -> 2 -> 1
        assert_eq!(result.unwrap().path, vec![1, 2, 1]);
    }

    #[test]
    fn test_detect_cycle_diamond() {
        // Current:
        // - 3 depends on 1 and 2 (3 -> 1, 3 -> 2)
        // - 4 depends on 1 and 2 (4 -> 1, 4 -> 2)
        // Adding "1 depends on 4" (1 -> 4) would create: 4 -> 1 -> 4
        let mut deps = HashMap::new();
        deps.insert(3, vec![1, 2]);
        deps.insert(4, vec![1, 2]);

        // Check if adding "1 depends on 4" creates a cycle
        // Does 4 have a path to 1? Yes, directly!
        let result = detect_cycle(1, 4, &deps).unwrap();
        assert!(result.is_some());
        // Cycle: 4 -> 1 -> 4
        assert_eq!(result.unwrap().path, vec![4, 1, 4]);
    }

    #[test]
    fn test_detect_cycle_chain() {
        // Current: 5 -> 4 -> 3 -> 2 -> 1
        // Adding "1 depends on 5" (1 -> 5) would create a full cycle
        let mut deps = HashMap::new();
        deps.insert(2, vec![1]);
        deps.insert(3, vec![2]);
        deps.insert(4, vec![3]);
        deps.insert(5, vec![4]);

        let result = detect_cycle(1, 5, &deps).unwrap();
        assert!(result.is_some());
        // Cycle: 5 -> 4 -> 3 -> 2 -> 1 -> 5
        assert_eq!(result.unwrap().path, vec![5, 4, 3, 2, 1, 5]);
    }
}
