use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::db::Database;
use crate::error::{Error, Result};
use crate::models::{DependencyInfo, Status, Task, TaskWithDeps};

#[derive(Clone)]
struct F64Wrapper(f64);

impl PartialEq for F64Wrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for F64Wrapper {}

impl PartialOrd for F64Wrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for F64Wrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

pub struct CoreImpl {
    pub db: Database,
}

impl CoreImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn topological_sort(&self, tasks: Vec<Task>) -> Result<(Vec<Task>, Vec<String>)> {
        if tasks.is_empty() {
            return Ok((vec![], vec![]));
        }

        let task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();

        let mut in_degree: HashMap<i64, usize> = tasks.iter().map(|t| (t.id, 0)).collect();
        let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();

        for task in &tasks {
            let deps = self.db.get_dependencies(task.id)?;
            let valid_deps: Vec<i64> = deps.into_iter().filter(|d| task_ids.contains(d)).collect();

            for dep in &valid_deps {
                *in_degree.entry(task.id).or_insert(0) += 1;
                adjacency.entry(*dep).or_default().push(task.id);
            }
        }

        let mut heap: BinaryHeap<(F64Wrapper, i64)> = BinaryHeap::new();

        for task in &tasks {
            if *in_degree.get(&task.id).unwrap_or(&0) == 0 {
                heap.push((F64Wrapper(task.manual_order), task.id));
            }
        }

        let mut sorted = Vec::new();

        while let Some((F64Wrapper(_order), id)) = heap.pop() {
            let task = tasks.iter().find(|t| t.id == id).unwrap().clone();
            sorted.push(task);

            if let Some(deps) = adjacency.get(&id) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            let dep_task = tasks.iter().find(|t| t.id == *dep).unwrap();
                            heap.push((F64Wrapper(dep_task.manual_order), *dep));
                        }
                    }
                }
            }
        }

        if sorted.len() != tasks.len() {
            return Err(Error::CycleDetected(
                0,
                0,
                "Cycle detected in dependency graph".to_string(),
            ));
        }

        let warnings = self.check_order_conflicts(&sorted);

        Ok((sorted, warnings))
    }

    fn check_order_conflicts(&self, tasks: &[Task]) -> Vec<String> {
        let mut warnings = Vec::new();

        for task in tasks {
            if let Ok(deps) = self.db.get_dependencies(task.id) {
                for dep_id in deps {
                    if let Ok(dep_task) = self.db.get_task(dep_id) {
                        if dep_task.manual_order > task.manual_order {
                            warnings.push(format!(
                                "Warning: #{} (order {}) depends on #{} (order {}) which has higher manual_order",
                                task.id, task.manual_order, dep_task.id, dep_task.manual_order
                            ));
                        }
                    }
                }
            }
        }

        warnings
    }

    pub fn get_task_with_deps(&self, id: i64) -> Result<TaskWithDeps> {
        let task = self.db.get_task(id)?;

        let dep_ids = self.db.get_dependencies(id)?;
        let mut dependencies = Vec::new();

        for dep_id in dep_ids {
            if let Ok(dep_task) = self.db.get_task(dep_id) {
                dependencies.push(DependencyInfo {
                    id: dep_task.id,
                    title: dep_task.title,
                    status: dep_task.status,
                });
            }
        }

        let dependents = self.db.get_dependents(id)?;

        Ok(TaskWithDeps {
            task,
            dependencies,
            dependents,
        })
    }

    pub fn add_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<Task> {
        self.db
            .create_task(title, description, dod, after_id, before_id)
    }

    pub fn edit_task(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        dod: Option<&str>,
    ) -> Result<Task> {
        self.db.update_task(id, title, description, dod)
    }

    pub fn show_task(&self, id: i64) -> Result<TaskWithDeps> {
        self.get_task_with_deps(id)
    }

    pub fn list_tasks(&self, all: bool) -> Result<(Vec<Task>, Option<i64>, Vec<String>)> {
        let target_id = self.db.get_target()?;

        let tasks = if all {
            self.db.get_all_tasks(crate::models::TaskFilter::all())?
        } else {
            match target_id {
                Some(tid) => self.db.get_target_subgraph(tid)?,
                None => return Err(Error::NoTarget),
            }
        };

        let (sorted, warnings) = self.topological_sort(tasks)?;

        Ok((sorted, target_id, warnings))
    }

    pub fn set_target(&self, id: i64) -> Result<()> {
        self.db.set_target(id)
    }

    pub fn get_target(&self) -> Result<Option<i64>> {
        self.db.get_target()
    }

    pub fn start_task(&self, id: i64) -> Result<Task> {
        let task = self.db.get_task(id)?;

        if task.status == Status::InProgress {
            return Ok(task);
        }

        if task.status == Status::Completed {
            return Err(Error::TaskAlreadyCompleted(id));
        }

        if task.status == Status::Blocked {
            return Err(Error::TaskNotPending(id));
        }

        if let Ok(active) = self.db.get_active_task() {
            return Err(Error::AnotherTaskActive(active.id, active.title));
        }

        let deps = self.db.get_dependencies(id)?;
        let unmet: Vec<i64> = deps
            .into_iter()
            .filter(|d| {
                if let Ok(t) = self.db.get_task(*d) {
                    t.status != Status::Completed
                } else {
                    false
                }
            })
            .collect();

        if let Some(first_unmet) = unmet.first() {
            return Err(Error::UnmetDependencies(id, *first_unmet));
        }

        self.db
            .update_task_status(id, Status::InProgress, true, false)
    }

    pub fn stop_task(&self) -> Result<Task> {
        let task = self.db.get_active_task()?;
        self.db
            .update_task_status(task.id, Status::Pending, false, false)
    }

    pub fn complete_task(&self) -> Result<Task> {
        let task = self.db.get_active_task()?;

        if task.dod.is_none() || task.dod.as_ref().map(|s| s.trim()).unwrap_or("").is_empty() {
            return Err(Error::NoDod(task.id));
        }

        self.db
            .update_task_status(task.id, Status::Completed, false, true)
    }

    pub fn block_task(&self, id: i64) -> Result<Task> {
        let task = self.db.get_task(id)?;

        if task.status != Status::Pending && task.status != Status::InProgress {
            return Err(Error::TaskNotPending(id));
        }

        if task.status == Status::InProgress {
            let active = self.db.get_active_task().ok();
            if let Some(a) = active {
                if a.id == id {
                    self.db
                        .update_task_status(id, Status::Blocked, false, false)?;
                }
            }
        }

        self.db
            .update_task_status(id, Status::Blocked, false, false)
    }

    pub fn unblock_task(&self, id: i64) -> Result<Task> {
        let task = self.db.get_task(id)?;

        if task.status != Status::Blocked {
            return Err(Error::TaskNotBlocked(id));
        }

        self.db
            .update_task_status(id, Status::Pending, false, false)
    }

    pub fn current_task(&self) -> Result<TaskWithDeps> {
        let task = self.db.get_active_task()?;
        self.get_task_with_deps(task.id)
    }

    pub fn next_task(&self) -> Result<(Option<Task>, Vec<i64>)> {
        let target_id = self.db.get_target()?;

        let target_id = match target_id {
            Some(t) => t,
            None => return Err(Error::NoTarget),
        };

        let tasks = self.db.get_target_subgraph(target_id)?;

        if tasks.is_empty() || tasks.iter().all(|t| t.status == Status::Completed) {
            return Err(Error::TargetReached(target_id));
        }

        let (sorted, _) = self.topological_sort(tasks)?;

        let pending: Vec<&Task> = sorted
            .iter()
            .filter(|t| t.status == Status::Pending)
            .collect();

        let available: Vec<&&Task> = pending
            .iter()
            .filter(|t| {
                let deps = self.db.get_dependencies(t.id).unwrap_or_default();
                deps.iter().all(|d| {
                    if let Ok(dt) = self.db.get_task(*d) {
                        dt.status == Status::Completed
                    } else {
                        true
                    }
                })
            })
            .collect();

        if !available.is_empty() {
            let task = (*available[0]).clone();
            Ok((Some(task), vec![]))
        } else {
            let blocked: Vec<i64> = pending.iter().map(|t| t.id).collect();
            if !blocked.is_empty() {
                Err(Error::AllBlocked(
                    blocked
                        .iter()
                        .map(|i| i.to_string())
                        .collect::<Vec<_>>()
                        .join(", #"),
                ))
            } else {
                Err(Error::TargetReached(target_id))
            }
        }
    }

    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        let _ = self.db.get_task(task_id)?;
        let _ = self.db.get_task(depends_on)?;

        if let Some(cycle) = self.db.check_cycle(task_id, depends_on)? {
            let cycle_str = cycle
                .iter()
                .map(|i| format!("#{i}"))
                .collect::<Vec<_>>()
                .join(" → ");
            return Err(Error::CycleDetected(task_id, depends_on, cycle_str));
        }

        self.db.add_dependency(task_id, depends_on)
    }

    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        self.db.remove_dependency(task_id, depends_on)
    }

    pub fn log_artifact(&self, name: &str, file_path: &str) -> Result<crate::models::Artifact> {
        let task = self.db.get_active_task()?;
        self.db.create_artifact(task.id, name, file_path)
    }

    pub fn get_artifacts(&self, task_id: Option<i64>) -> Result<Vec<crate::models::Artifact>> {
        self.db.get_artifacts(task_id)
    }

    pub fn reorder_task(
        &self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<f64> {
        self.db.reorder_task(id, after_id, before_id)
    }

    pub fn reindex(&self) -> Result<()> {
        self.db.reindex()
    }
}
