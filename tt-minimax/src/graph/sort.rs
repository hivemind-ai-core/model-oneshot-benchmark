use crate::db::{Db, Task, TaskStatus};
use crate::error::{Error, Result};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Wrapper for BinaryHeap to make it a min-heap based on manual_order
/// Note: f64 doesn't implement Eq, so we use ordered_float for comparison
#[derive(Debug, Clone)]
struct MinHeapItem {
    task_id: i64,
    manual_order: f64,
}

impl PartialEq for MinHeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.task_id == other.task_id && self.manual_order.to_bits() == other.manual_order.to_bits()
    }
}

impl Eq for MinHeapItem {}

impl Ord for MinHeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap behavior
        match other.manual_order.partial_cmp(&self.manual_order) {
            Some(Ordering::Equal) | None => other.task_id.cmp(&self.task_id),
            Some(ord) => ord.then_with(|| other.task_id.cmp(&self.task_id)),
        }
    }
}

impl PartialOrd for MinHeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Get the target subgraph using recursive CTE
/// Returns all tasks that are transitive dependencies of the target
pub fn get_target_subgraph(db: &Db, target_id: i64) -> Result<Vec<i64>> {
    let mut stmt = db.conn.prepare(
        "WITH RECURSIVE dep_graph(task_id) AS (
            -- Start with the target
            SELECT ?1
            UNION ALL
            -- Add all dependencies
            SELECT d.depends_on
            FROM dep_graph g
            JOIN dependencies d ON d.task_id = g.task_id
        )
        SELECT DISTINCT task_id FROM dep_graph",
    )?;

    let ids = stmt
        .query_map([target_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<i64>, _>>()?;

    Ok(ids)
}

/// Get the active subgraph (excluding completed tasks)
pub fn get_active_subgraph(db: &Db, target_id: i64) -> Result<Vec<i64>> {
    let subgraph = get_target_subgraph(db, target_id)?;
    let mut active = Vec::new();

    for id in subgraph {
        let status = get_task_status(db, id)?;
        if status != TaskStatus::Completed {
            active.push(id);
        }
    }

    Ok(active)
}

/// Get the status of a task
fn get_task_status(db: &Db, id: i64) -> Result<TaskStatus> {
    let mut stmt = db.conn.prepare("SELECT status FROM tasks WHERE id = ?1")?;
    let status_str: String = stmt.query_row([id], |row| row.get(0))?;
    TaskStatus::from_str(&status_str)
}

/// Perform topological sort using Kahn's algorithm with priority queue
/// Returns tasks sorted by dependency order, then by manual_order
pub fn topological_sort(db: &Db, task_ids: &[i64]) -> Result<Vec<i64>> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build in-degree map and adjacency list
    let mut in_degree: HashMap<i64, i64> = HashMap::new();
    let mut adj: HashMap<i64, Vec<i64>> = HashMap::new();
    let task_set: HashSet<i64> = task_ids.iter().cloned().collect();

    // Initialize in-degrees to 0
    for &id in task_ids {
        in_degree.insert(id, 0);
    }

    // Build in-degree and adjacency
    for &id in task_ids {
        // Get dependencies of this task that are also in our task set
        let mut dep_stmt = db
            .conn
            .prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;

        let deps = dep_stmt
            .query_map([id], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<i64>, _>>()?;

        // Only count dependencies that are in our task set
        let active_deps: Vec<i64> = deps
            .into_iter()
            .filter(|dep| task_set.contains(dep))
            .collect();

        in_degree.insert(id, active_deps.len() as i64);

        for dep in active_deps {
            adj.entry(dep).or_default().push(id);
        }
    }

    // Seed min-heap with tasks that have no dependencies (in our subgraph)
    let mut heap: BinaryHeap<MinHeapItem> = BinaryHeap::new();
    for &id in task_ids {
        if let Some(&degree) = in_degree.get(&id) {
            if degree == 0 {
                let order = get_manual_order(db, id)?;
                heap.push(MinHeapItem {
                    task_id: id,
                    manual_order: order,
                });
            }
        }
    }

    // Kahn's algorithm
    let mut result = Vec::new();
    while let Some(item) = heap.pop() {
        result.push(item.task_id);

        if let Some(neighbors) = adj.get(&item.task_id) {
            for &neighbor in neighbors {
                if let Some(degree) = in_degree.get_mut(&neighbor) {
                    *degree -= 1;
                    if *degree == 0 {
                        let order = get_manual_order(db, neighbor)?;
                        heap.push(MinHeapItem {
                            task_id: neighbor,
                            manual_order: order,
                        });
                    }
                }
            }
        }
    }

    // Check for cycles
    if result.len() != task_ids.len() {
        return Err(Error::CycleDetected {
            from_id: 0,
            to_id: 0,
            cycle_path: vec![],
        });
    }

    Ok(result)
}

/// Get the manual_order of a task
fn get_manual_order(db: &Db, id: i64) -> Result<f64> {
    let mut stmt = db
        .conn
        .prepare("SELECT manual_order FROM tasks WHERE id = ?1")?;
    let order: f64 = stmt.query_row([id], |row| row.get(0))?;
    Ok(order)
}

/// Check for order conflicts - tasks that have lower manual_order than their prerequisites
pub fn check_order_conflicts(db: &Db, sorted_ids: &[i64]) -> Result<Vec<(i64, f64, i64, f64)>> {
    let mut conflicts = Vec::new();

    let task_orders: HashMap<i64, f64> = sorted_ids
        .iter()
        .map(|&id| Ok((id, get_manual_order(db, id)?)))
        .collect::<Result<HashMap<_, _>>>()?;

    for &id in sorted_ids {
        // Get dependencies of this task
        let mut stmt = db
            .conn
            .prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;

        let deps = stmt
            .query_map([id], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<i64>, _>>()?;

        let task_order = task_orders.get(&id).copied().unwrap_or(0.0);

        for dep_id in deps {
            if let Some(&dep_order) = task_orders.get(&dep_id) {
                // Check if task has lower manual_order than its dependency
                if task_order < dep_order {
                    conflicts.push((id, task_order, dep_id, dep_order));
                }
            }
        }
    }

    Ok(conflicts)
}

/// Get the next task to work on
/// Returns the first pending task whose dependencies are all completed
pub fn get_next_task(db: &Db, target_id: i64) -> Result<Task> {
    let active_ids = get_active_subgraph(db, target_id)?;

    if active_ids.is_empty() {
        // Get the target task for the error message
        let target = get_task(db, target_id)?;
        return Err(Error::TargetReached {
            id: target_id,
            title: target.title,
        });
    }

    let sorted = topological_sort(db, &active_ids)?;

    // Find the first pending task with all dependencies completed
    for id in sorted {
        let task = get_task(db, id)?;
        if task.status == TaskStatus::Pending {
            // Check if all dependencies are completed
            let all_completed = are_dependencies_completed(db, id)?;
            if all_completed {
                return Ok(task);
            }
        }
    }

    // All remaining tasks are blocked
    let blocked: Vec<i64> = active_ids
        .into_iter()
        .filter(|&id| {
            if let Ok(task) = get_task(db, id) {
                task.status == TaskStatus::Blocked
            } else {
                false
            }
        })
        .collect();

    Err(Error::AllBlocked {
        blocked_ids: blocked,
    })
}

/// Get a task by ID
fn get_task(db: &Db, id: i64) -> Result<Task> {
    let mut stmt = db.conn.prepare(
        "SELECT id, title, description, dod, status, manual_order,
                created_at, started_at, completed_at, last_touched_at
         FROM tasks WHERE id = ?1",
    )?;

    let task = stmt.query_row([id], |row| {
        let status_str: String = row.get(4)?;
        let status = TaskStatus::from_str(&status_str)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        Ok(Task {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            dod: row.get(3)?,
            status,
            manual_order: row.get(5)?,
            created_at: row.get(6)?,
            started_at: row.get(7)?,
            completed_at: row.get(8)?,
            last_touched_at: row.get(9)?,
        })
    })?;

    Ok(task)
}

/// Check if all dependencies of a task are completed
fn are_dependencies_completed(db: &Db, task_id: i64) -> Result<bool> {
    let mut stmt = db.conn.prepare(
        "SELECT d.depends_on, t.status
         FROM dependencies d
         JOIN tasks t ON t.id = d.depends_on
         WHERE d.task_id = ?1",
    )?;

    let rows = stmt.query_map([task_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (_dep_id, status_str) = row?;
        let status = TaskStatus::from_str(&status_str)?;
        if status != TaskStatus::Completed {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Calculate the midpoint between two manual_order values
pub fn calculate_midpoint(a: f64, b: f64) -> Result<f64> {
    let midpoint = (a + b) / 2.0;

    // Check for precision exhaustion
    if midpoint == a || midpoint == b {
        return Err(Error::FloatPrecisionExhausted { a, b });
    }

    Ok(midpoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_midpoint() {
        assert_eq!(calculate_midpoint(10.0, 20.0).unwrap(), 15.0);
        assert_eq!(calculate_midpoint(0.0, 10.0).unwrap(), 5.0);
        assert_eq!(calculate_midpoint(-10.0, 10.0).unwrap(), 0.0);
    }

    #[test]
    fn test_precision_exhaustion() {
        // Very close values that might result in precision exhaustion
        let a: f64 = 1.0;
        let b = f64::from_bits(a.to_bits() + 1); // Next representable float
        let result = calculate_midpoint(a, b);
        // This should likely fail due to precision exhaustion
        assert!(result.is_err());
    }
}
