//! Scheduler integration coverage.
use agentforge_core::agent::{AgentRole, AgentTask};
use agentforge_core::task::TaskGraph;
use agentforge_scheduler::{ScheduleError, plan_batch, reserve_integrator};

#[test]
fn integrator_reservation_is_serialized() {
    let a = AgentTask::new("a", "m", AgentRole::Integrator, "merge");
    let b = AgentTask::new("b", "m", AgentRole::Integrator, "merge");
    let graph = TaskGraph::from_tasks([a, b]).unwrap();
    assert_eq!(
        reserve_integrator(&graph),
        Err(ScheduleError::MultipleIntegrators)
    );
}

#[test]
fn empty_graph_has_empty_batch() {
    let graph = TaskGraph::from_tasks(std::iter::empty::<AgentTask>()).unwrap();
    assert!(plan_batch(&graph).unwrap().task_ids.is_empty());
}
