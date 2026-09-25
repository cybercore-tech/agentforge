//! Opt-in automatic dispatch of ready tasks to registered workers (P4-M006).
//!
//! Nothing is granted automatically unless `.forge/dispatch.conf` enables it. A pass grants
//! pending, ready tasks in the listed milestones, in task-ID order, through the same
//! [`grant_lease`](crate::leases::grant_lease) path as a manual grant. It only grants tasks whose
//! pre-execution approvals are recorded and that never held a lease (dispatch-once), and it stops
//! at worker capacity or `max_per_tick`.

use crate::leases::{LeaseView, grant_lease_as, load_book, load_workers, validate_ttl};
use crate::{OperatorError, approved_boundaries, load_graph, validate_actor};
use agentforge_core::task::TaskState;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

/// Project-relative dispatch policy path.
pub const DISPATCH_POLICY_RELATIVE_PATH: &str = ".forge/dispatch.conf";
/// Default automatic lease TTL (15 minutes).
pub const DEFAULT_DISPATCH_TTL_MS: u64 = 15 * 60 * 1_000;
/// Default and maximum grants per pass.
pub const DEFAULT_MAX_PER_TICK: usize = 4;
const MAX_PER_TICK_LIMIT: usize = 64;
const MAX_POLICY_BYTES: u64 = 4 * 1024;
const MAX_MILESTONE_BYTES: usize = 32;

/// A reviewed automatic-dispatch policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchPolicy {
    /// Whether automatic dispatch is on.
    pub enabled: bool,
    /// Only tasks in these milestones are dispatched.
    pub milestones: BTreeSet<String>,
    /// Lease window for automatic grants.
    pub ttl_ms: u64,
    /// Maximum grants per pass.
    pub max_per_tick: usize,
}

impl DispatchPolicy {
    /// Loads `.forge/dispatch.conf`. A missing file returns `None` (dispatch off); an invalid file
    /// is an error.
    pub fn load(root: impl AsRef<Path>) -> Result<Option<Self>, OperatorError> {
        let path = root.as_ref().join(DISPATCH_POLICY_RELATIVE_PATH);
        let invalid = |reason: String| {
            OperatorError::new(format!("dispatch policy {}: {reason}", path.display()))
        };
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(invalid(error.to_string())),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(invalid("is not a regular file".into()));
        }
        if metadata.len() > MAX_POLICY_BYTES {
            return Err(invalid(format!("exceeds {MAX_POLICY_BYTES} bytes")));
        }
        let text = fs::read_to_string(&path).map_err(|error| invalid(error.to_string()))?;
        let mut enabled = None;
        let mut milestones = BTreeSet::new();
        let mut ttl_ms = None;
        let mut max_per_tick = None;
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| invalid(format!("line {} is not key=value", index + 1)))?;
            let value = value.trim();
            match key.trim() {
                "enabled" if enabled.is_none() => {
                    enabled = Some(match value {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(invalid(format!("enabled must be true or false: {other}")));
                        }
                    });
                }
                "milestone" => {
                    if value.is_empty()
                        || value.len() > MAX_MILESTONE_BYTES
                        || !value
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '-')
                    {
                        return Err(invalid(format!("invalid milestone: {value}")));
                    }
                    milestones.insert(value.to_owned());
                }
                "ttl_ms" if ttl_ms.is_none() => {
                    let parsed = value
                        .parse::<u64>()
                        .map_err(|_| invalid(format!("ttl_ms is not a number: {value}")))?;
                    validate_ttl(parsed).map_err(|error| invalid(error.to_string()))?;
                    ttl_ms = Some(parsed);
                }
                "max_per_tick" if max_per_tick.is_none() => {
                    let parsed = value
                        .parse::<usize>()
                        .ok()
                        .filter(|count| (1..=MAX_PER_TICK_LIMIT).contains(count))
                        .ok_or_else(|| {
                            invalid(format!(
                                "max_per_tick must be 1..={MAX_PER_TICK_LIMIT}: {value}"
                            ))
                        })?;
                    max_per_tick = Some(parsed);
                }
                other => return Err(invalid(format!("unknown or repeated key: {other}"))),
            }
        }
        let enabled = enabled.ok_or_else(|| invalid("enabled is missing".into()))?;
        if milestones.is_empty() {
            return Err(invalid("at least one milestone= is required".into()));
        }
        Ok(Some(Self {
            enabled,
            milestones,
            ttl_ms: ttl_ms.unwrap_or(DEFAULT_DISPATCH_TTL_MS),
            max_per_tick: max_per_tick.unwrap_or(DEFAULT_MAX_PER_TICK),
        }))
    }
}

/// The outcome of one dispatch pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DispatchPass {
    /// Whether an enabled policy was in force.
    pub enabled: bool,
    /// Leases granted in this pass, in task-ID order.
    pub granted: Vec<LeaseView>,
    /// Candidate tasks not granted, with the reason, in task-ID order.
    pub skipped: Vec<(String, String)>,
    /// Why the pass stopped early, if it did (capacity, per-tick limit, no workers).
    pub stopped: Option<String>,
}

/// Runs one automatic-dispatch pass at `now` on behalf of `actor`.
pub fn dispatch_ready(
    root: impl AsRef<Path>,
    now: u64,
    actor: &str,
) -> Result<DispatchPass, OperatorError> {
    validate_actor(actor)?;
    let root = root.as_ref();
    let Some(policy) = DispatchPolicy::load(root)?.filter(|policy| policy.enabled) else {
        return Ok(DispatchPass::default());
    };
    let mut pass = DispatchPass {
        enabled: true,
        ..DispatchPass::default()
    };
    let workers = load_workers(root)?;
    if workers.is_empty() {
        pass.stopped = Some("no workers are registered".into());
        return Ok(pass);
    }
    let graph = load_graph(root)?;
    let mut ready = graph
        .ready_task_ids()
        .map_err(|error| OperatorError::new(error.to_string()))?;
    ready.sort();
    let ever_leased = load_book(root)?
        .leases()
        .map(|lease| lease.task_id().clone())
        .collect::<BTreeSet<_>>();
    for task_id in ready {
        let Some(record) = graph.get(&task_id) else {
            continue;
        };
        if record.state() != TaskState::Pending
            || !policy.milestones.contains(&record.task().milestone_id)
        {
            continue;
        }
        if ever_leased.contains(&task_id) {
            pass.skipped.push((
                task_id.to_string(),
                "already had a lease; automatic dispatch runs once (grant manually to retry)"
                    .into(),
            ));
            continue;
        }
        let approved = approved_boundaries(root, &task_id)?;
        if let Some(missing) = record
            .task()
            .required_approvals
            .iter()
            .find(|boundary| !boundary.is_post_execution() && !approved.contains(boundary))
        {
            pass.skipped.push((
                task_id.to_string(),
                format!("approval {} is not recorded", missing.as_str()),
            ));
            continue;
        }
        if pass.granted.len() >= policy.max_per_tick {
            pass.stopped = Some(format!("max_per_tick {} reached", policy.max_per_tick));
            break;
        }
        let book = load_book(root)?;
        let has_capacity = workers.iter().any(|worker| {
            book.active_leases_at(now)
                .iter()
                .filter(|lease| lease.worker_id() == worker.worker_id())
                .count()
                < usize::from(worker.max_concurrent_leases())
        });
        if !has_capacity {
            pass.stopped = Some("every registered worker is at capacity".into());
            break;
        }
        match grant_lease_as(
            root,
            &task_id,
            None,
            policy.ttl_ms,
            now,
            actor,
            &[("dispatch", "auto")],
        ) {
            Ok(lease) => pass.granted.push(lease),
            Err(error) => pass.skipped.push((task_id.to_string(), error.to_string())),
        }
    }
    Ok(pass)
}
