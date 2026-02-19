use crate::db::Db;
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};

/// Check if adding a dependency from `from_id` to `to_id` would create a cycle.
/// Returns the full cycle path if a cycle would be created.
pub fn check_cycle(db: &Db, from_id: i64, to_id: i64) -> Result<()> {
    // DFS from to_id following dependencies
    // If we can reach from_id, there's a cycle

    let mut visited: HashSet<i64> = HashSet::new();
    let mut path: Vec<i64> = Vec::new();
    let mut stack: Vec<(i64, bool)> = vec![(to_id, false)]; // (node, processed)

    while let Some((current, processed)) = stack.pop() {
        if processed {
            path.pop();
            continue;
        }

        if visited.contains(&current) {
            continue;
        }

        visited.insert(current);
        path.push(current);

        if current == from_id {
            // Found a cycle - return the full cycle path
            path.push(to_id); // Complete the cycle
            return Err(Error::CycleDetected {
                from_id,
                to_id,
                cycle_path: path,
            });
        }

        // Add this node again to be popped later
        stack.push((current, true));

        // Get all tasks that `current` depends on
        let deps = get_dependencies(db, current)?;
        for dep in deps {
            if !visited.contains(&dep) {
                stack.push((dep, false));
            }
        }
    }

    Ok(())
}

/// Get all tasks that the given task depends on (direct dependencies)
fn get_dependencies(db: &Db, task_id: i64) -> Result<Vec<i64>> {
    let mut stmt = db
        .conn
        .prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;

    let deps = stmt
        .query_map([task_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<i64>, _>>()?;

    Ok(deps)
}

/// Detect if there are any cycles in the entire dependency graph
/// Returns a list of cycle paths found
pub fn detect_all_cycles(db: &Db) -> Result<Vec<Vec<i64>>> {
    let mut cycles = Vec::new();
    let task_ids = get_all_task_ids(db)?;

    // Build adjacency list for the graph
    let adj = build_adjacency_list(db)?;

    for &start in &task_ids {
        let mut visited: HashSet<i64> = HashSet::new();
        let mut rec_stack: HashSet<i64> = HashSet::new();
        let mut path: Vec<i64> = Vec::new();

        if dfs_detect_cycle(&adj, start, &mut visited, &mut rec_stack, &mut path) {
            // Found a cycle starting from this node
            if let Some(cycle_path) = extract_cycle_path(&path, start) {
                if !cycles.contains(&cycle_path) {
                    cycles.push(cycle_path);
                }
            }
        }
    }

    Ok(cycles)
}

/// Get all task IDs in the database
fn get_all_task_ids(db: &Db) -> Result<Vec<i64>> {
    let mut stmt = db.conn.prepare("SELECT id FROM tasks")?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<i64>, _>>()?;
    Ok(ids)
}

/// Build adjacency list for the graph
fn build_adjacency_list(db: &Db) -> Result<HashMap<i64, Vec<i64>>> {
    let mut adj: HashMap<i64, Vec<i64>> = HashMap::new();

    let mut stmt = db
        .conn
        .prepare("SELECT task_id, depends_on FROM dependencies")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;

    for row in rows {
        let (task_id, dep_id) = row?;
        adj.entry(dep_id).or_insert_with(Vec::new).push(task_id);
    }

    Ok(adj)
}

/// DFS to detect cycles
fn dfs_detect_cycle(
    adj: &HashMap<i64, Vec<i64>>,
    node: i64,
    visited: &mut HashSet<i64>,
    rec_stack: &mut HashSet<i64>,
    path: &mut Vec<i64>,
) -> bool {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node);

    if let Some(neighbors) = adj.get(&node) {
        for &neighbor in neighbors {
            if !visited.contains(&neighbor) {
                if dfs_detect_cycle(adj, neighbor, visited, rec_stack, path) {
                    return true;
                }
            } else if rec_stack.contains(&neighbor) {
                // Found a cycle
                path.push(neighbor);
                return true;
            }
        }
    }

    rec_stack.remove(&node);
    path.pop();
    false
}

/// Extract the cycle path from a DFS path
fn extract_cycle_path(path: &[i64], start: i64) -> Option<Vec<i64>> {
    // Find the last occurrence of start in the path
    if let Some(pos) = path.iter().rposition(|&x| x == start) {
        let cycle: Vec<i64> = path[pos..].to_vec();
        if cycle.len() > 1 {
            return Some(cycle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cycle_simple() {
        // This would need a full test setup with database
        // For now, we'll test the logic separately
    }
}
