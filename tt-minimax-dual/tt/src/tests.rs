#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::core::CoreImpl;
    use crate::db::Database;
    use crate::models::Status;

    fn setup_test() -> (TempDir, CoreImpl) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("tt.db");
        let db = Database::new(&db_path).unwrap();
        let core = CoreImpl::new(db);
        (temp_dir, core)
    }

    #[test]
    fn test_create_task() {
        let (_temp, core) = setup_test();
        let task = core
            .add_task("Test Task", Some("Description"), Some("DoD"), None, None)
            .unwrap();
        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, Some("Description".to_string()));
        assert_eq!(task.dod, Some("DoD".to_string()));
        assert_eq!(task.status, Status::Pending);
    }

    #[test]
    fn test_edit_task() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Original", None, None, None, None).unwrap();
        let edited = core
            .edit_task(task.id, Some("Updated"), Some("New desc"), Some("New DoD"))
            .unwrap();
        assert_eq!(edited.title, "Updated");
        assert_eq!(edited.description, Some("New desc".to_string()));
        assert_eq!(edited.dod, Some("New DoD".to_string()));
    }

    #[test]
    fn test_start_task() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Task", None, None, None, None).unwrap();
        let started = core.start_task(task.id).unwrap();
        assert_eq!(started.status, Status::InProgress);
    }

    #[test]
    fn test_start_already_in_progress_is_noop() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Task", None, None, None, None).unwrap();
        core.start_task(task.id).unwrap();
        let result = core.start_task(task.id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_complete_requires_dod() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Task", None, None, None, None).unwrap();
        core.start_task(task.id).unwrap();
        let result = core.complete_task();
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_with_dod() {
        let (_temp, core) = setup_test();
        let task = core
            .add_task("Task", None, Some("DoD"), None, None)
            .unwrap();
        core.start_task(task.id).unwrap();
        let completed = core.complete_task().unwrap();
        assert_eq!(completed.status, Status::Completed);
    }

    #[test]
    fn test_dependency() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();
        let task2 = core
            .add_task("Task 2", None, Some("DoD2"), None, None)
            .unwrap();

        core.add_dependency(task2.id, task1.id).unwrap();

        let deps = core.db.get_dependencies(task2.id).unwrap();
        assert!(deps.contains(&task1.id));
    }

    #[test]
    fn test_cycle_detection() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();
        let task2 = core
            .add_task("Task 2", None, Some("DoD2"), None, None)
            .unwrap();

        core.add_dependency(task2.id, task1.id).unwrap();

        let result = core.add_dependency(task1.id, task2.id);
        assert!(result.is_err());
    }

    #[test]
    fn test_target_system() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();
        let task2 = core
            .add_task("Task 2", None, Some("DoD2"), None, None)
            .unwrap();

        core.add_dependency(task2.id, task1.id).unwrap();
        core.set_target(task2.id).unwrap();

        let target = core.get_target().unwrap();
        assert_eq!(target, Some(task2.id));
    }

    #[test]
    fn test_topological_sort() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD"), None, None)
            .unwrap();
        let task2 = core
            .add_task("Task 2", None, Some("DoD"), None, None)
            .unwrap();
        let task3 = core
            .add_task("Task 3", None, Some("DoD"), None, None)
            .unwrap();

        core.add_dependency(task2.id, task1.id).unwrap();
        core.add_dependency(task3.id, task2.id).unwrap();

        let (sorted, _) = core
            .topological_sort(vec![task3.clone(), task2.clone(), task1.clone()])
            .unwrap();

        let ids: Vec<i64> = sorted.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![task1.id, task2.id, task3.id]);
    }

    #[test]
    fn test_next_task_with_dependencies() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();
        let task2 = core
            .add_task("Task 2", None, Some("DoD2"), None, None)
            .unwrap();

        core.add_dependency(task2.id, task1.id).unwrap();
        core.set_target(task2.id).unwrap();

        let (next, _) = core.next_task().unwrap();
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, task1.id);
    }

    #[test]
    fn test_block_unblock() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Task", None, None, None, None).unwrap();

        let blocked = core.block_task(task.id).unwrap();
        assert_eq!(blocked.status, Status::Blocked);

        let unblocked = core.unblock_task(task.id).unwrap();
        assert_eq!(unblocked.status, Status::Pending);
    }

    #[test]
    fn test_artifacts() {
        let (_temp, core) = setup_test();
        let task = core.add_task("Task", None, None, None, None).unwrap();
        core.start_task(task.id).unwrap();

        let artifact = core
            .log_artifact("research", ".tt/artifacts/research.md")
            .unwrap();
        assert_eq!(artifact.name, "research");

        let artifacts = core.get_artifacts(Some(task.id)).unwrap();
        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn test_target_reached() {
        let (_temp, core) = setup_test();
        let task1 = core
            .add_task("Task 1", None, Some("DoD1"), None, None)
            .unwrap();

        core.set_target(task1.id).unwrap();
        core.start_task(task1.id).unwrap();
        core.complete_task().unwrap();

        let result = core.next_task();
        assert!(result.is_err());
    }

    #[test]
    fn test_ordering_with_manual_order() {
        let (_temp, core) = setup_test();
        let task_a = core.add_task("A", None, Some("DoD"), None, None).unwrap();
        let task_b = core.add_task("B", None, Some("DoD"), None, None).unwrap();

        core.reorder_task(task_b.id, Some(task_a.id), None).unwrap();

        let tasks = core
            .db
            .get_all_tasks(crate::models::TaskFilter::all())
            .unwrap();
        assert_eq!(tasks[0].id, task_a.id);
        assert_eq!(tasks[1].id, task_b.id);
    }
}
