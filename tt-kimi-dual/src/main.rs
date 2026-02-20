use tt::cli;
use tt::core::error::TTError;

fn main() {
    if let Err(e) = cli::run() {
        match &e {
            TTError::NotInitialized => {
                eprintln!("Error: Not initialized. Run `tt init` first.");
            }
            TTError::AlreadyInitialized => {
                eprintln!("Error: Already initialized.");
            }
            TTError::TaskNotFound(id) => {
                eprintln!("Error: Task #{id} not found");
            }
            TTError::TaskNotPending(id) => {
                eprintln!("Error: Task #{id} is not pending, cannot start");
            }
            TTError::AnotherTaskActive(id) => {
                eprintln!("Error: Task #{id} is already in progress. Finish or stop it first.");
            }
            TTError::NoActiveTask => {
                eprintln!("Error: No task is currently in progress");
            }
            TTError::UnmetDependencies(id, deps) => {
                let dep_list: Vec<String> = deps.iter().map(|d| format!("#{d}")).collect();
                eprintln!(
                    "Error: Cannot start #{}: dependencies not completed: {}",
                    id,
                    dep_list.join(", ")
                );
            }
            TTError::CycleDetected(from, to, cycle) => {
                let cycle_str: Vec<String> = cycle.iter().map(|id| format!("#{id}")).collect();
                eprintln!(
                    "Error: Adding #{} → #{} would create a cycle: {}",
                    from,
                    to,
                    cycle_str.join(" → ")
                );
            }
            TTError::NoTarget => {
                eprintln!("Error: No target set. Use `tt target <id>` first.");
            }
            TTError::TargetReached(id) => {
                eprintln!("Target reached. All tasks for #{id} are completed.");
            }
            TTError::NoDod(id) => {
                eprintln!(
                    "Error: Task #{id} has no definition of done. Set one with `tt edit {id} --dod`"
                );
            }
            TTError::AllBlocked(ids) => {
                eprintln!("Error: All remaining tasks are blocked:");
                for id in ids {
                    eprintln!("  - #{id}: blocked");
                }
            }
            TTError::FloatPrecisionExhausted => {
                eprintln!("Error: Cannot calculate midpoint: float precision exhausted. Run `tt reindex` to fix.");
            }
            _ => {
                eprintln!("Error: {e}");
            }
        }
        std::process::exit(1);
    }
}
