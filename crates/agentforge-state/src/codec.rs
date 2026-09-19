use crate::{STATE_FORMAT_VERSION, StateError, StateSnapshot};
use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use agentforge_core::task_graph::{TaskGraph, TaskRecord, TaskState};

const MAGIC: &[u8; 4] = b"AFST";
const MAX_STRING_LEN: usize = 1024 * 1024;
const MAX_COLLECTION_LEN: usize = 100_000;

/// Encodes a validated state snapshot into the deterministic AgentForge durable-state format.
///
/// # Errors
///
/// Returns [`StateError`] when the snapshot is invalid or contains values that cannot be encoded
/// within the supported format bounds.
pub fn encode_snapshot(snapshot: &StateSnapshot) -> Result<Vec<u8>, StateError> {
    snapshot.validate()?;

    let mut body = Vec::new();

    write_len(&mut body, snapshot.graph.len(), "task collection")?;

    for (_, record) in snapshot.graph.iter() {
        write_string(&mut body, record.stable_key())?;
        write_string(&mut body, record.state().as_str())?;
        write_task(&mut body, record.task())?;
    }

    let checksum = checksum(&body);
    let body_len = u64::try_from(body.len()).map_err(|_| StateError::InvalidLength {
        kind: "snapshot body",
        value: u64::MAX,
    })?;

    let mut output = Vec::with_capacity(30 + body.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&STATE_FORMAT_VERSION.to_le_bytes());
    output.extend_from_slice(&snapshot.generation.to_le_bytes());
    output.extend_from_slice(&body_len.to_le_bytes());
    output.extend_from_slice(&checksum.to_le_bytes());
    output.extend_from_slice(&body);

    Ok(output)
}

/// Decodes and validates an AgentForge durable-state snapshot.
///
/// # Errors
///
/// Returns [`StateError`] when the input is truncated, corrupt, unsupported, malformed, or
/// reconstructs an invalid task graph.
pub fn decode_snapshot(bytes: &[u8]) -> Result<StateSnapshot, StateError> {
    let mut reader = Reader::new(bytes);

    if reader.take(4)? != MAGIC {
        return Err(StateError::InvalidMagic);
    }

    let version = reader.read_u16()?;

    if version != STATE_FORMAT_VERSION {
        return Err(StateError::UnsupportedVersion { found: version });
    }

    let generation = reader.read_u64()?;

    if generation == 0 {
        return Err(StateError::InvalidGeneration);
    }

    let body_len_u64 = reader.read_u64()?;
    let stored_checksum = reader.read_u64()?;

    let body_len = usize::try_from(body_len_u64).map_err(|_| StateError::InvalidLength {
        kind: "snapshot body",
        value: body_len_u64,
    })?;

    let body = reader.take(body_len)?;

    if !reader.is_empty() {
        return Err(StateError::TrailingData);
    }

    let calculated_checksum = checksum(body);

    if calculated_checksum != stored_checksum {
        return Err(StateError::ChecksumMismatch {
            expected: stored_checksum,
            found: calculated_checksum,
        });
    }

    let mut body_reader = Reader::new(body);
    let task_count = body_reader.read_len("task collection", MAX_COLLECTION_LEN)?;

    let mut graph = TaskGraph::new();

    for _ in 0..task_count {
        let stable_key = body_reader.read_string()?;
        let state = decode_task_state(&body_reader.read_string()?)?;
        let task = read_task(&mut body_reader)?;

        let record = TaskRecord::from_task(stable_key, task, state).map_err(StateError::Graph)?;

        graph.insert(record).map_err(StateError::Graph)?;
    }

    if !body_reader.is_empty() {
        return Err(StateError::TrailingData);
    }

    StateSnapshot::new(generation, graph)
}

fn write_task(output: &mut Vec<u8>, task: &AgentTask) -> Result<(), StateError> {
    output.extend_from_slice(&task.contract_version.to_le_bytes());
    write_string(output, &task.task_id)?;
    write_string(output, &task.milestone_id)?;
    write_string(output, task.primary_role.as_str())?;
    write_string(output, &task.goal)?;

    write_strings(output, &task.non_goals)?;
    write_strings(output, &task.dependency_task_ids)?;
    write_strings(output, &task.allowed_paths)?;
    write_strings(output, &task.forbidden_paths)?;

    write_len(output, task.capabilities.len(), "capability collection")?;
    for capability in &task.capabilities {
        write_string(output, capability.as_str())?;
    }

    write_len(output, task.required_approvals.len(), "approval collection")?;
    for approval in &task.required_approvals {
        write_string(output, approval.as_str())?;
    }

    write_strings(output, &task.required_gates)?;
    write_strings(output, &task.expected_outputs)?;
    write_strings(output, &task.evidence_requirements)?;

    Ok(())
}

fn read_task(reader: &mut Reader<'_>) -> Result<AgentTask, StateError> {
    let contract_version = reader.read_u16()?;
    let task_id = reader.read_string()?;
    let milestone_id = reader.read_string()?;
    let primary_role = decode_role(&reader.read_string()?)?;
    let goal = reader.read_string()?;

    let non_goals = reader.read_strings()?;
    let dependency_task_ids = reader.read_strings()?;
    let allowed_paths = reader.read_strings()?;
    let forbidden_paths = reader.read_strings()?;

    let capability_count = reader.read_len("capability collection", MAX_COLLECTION_LEN)?;
    let mut capabilities = Vec::with_capacity(capability_count);

    for _ in 0..capability_count {
        capabilities.push(decode_capability(&reader.read_string()?)?);
    }

    let approval_count = reader.read_len("approval collection", MAX_COLLECTION_LEN)?;
    let mut required_approvals = Vec::with_capacity(approval_count);

    for _ in 0..approval_count {
        required_approvals.push(decode_approval(&reader.read_string()?)?);
    }

    let required_gates = reader.read_strings()?;
    let expected_outputs = reader.read_strings()?;
    let evidence_requirements = reader.read_strings()?;

    Ok(AgentTask {
        contract_version,
        task_id,
        milestone_id,
        primary_role,
        goal,
        non_goals,
        dependency_task_ids,
        allowed_paths,
        forbidden_paths,
        capabilities,
        required_approvals,
        required_gates,
        expected_outputs,
        evidence_requirements,
    })
}

fn write_strings(output: &mut Vec<u8>, values: &[String]) -> Result<(), StateError> {
    write_len(output, values.len(), "string collection")?;

    for value in values {
        write_string(output, value)?;
    }

    Ok(())
}

fn write_string(output: &mut Vec<u8>, value: &str) -> Result<(), StateError> {
    if value.len() > MAX_STRING_LEN {
        return Err(StateError::InvalidLength {
            kind: "string",
            value: u64::try_from(value.len()).unwrap_or(u64::MAX),
        });
    }

    write_len(output, value.len(), "string")?;
    output.extend_from_slice(value.as_bytes());

    Ok(())
}

fn write_len(output: &mut Vec<u8>, value: usize, kind: &'static str) -> Result<(), StateError> {
    let encoded = u32::try_from(value).map_err(|_| StateError::InvalidLength {
        kind,
        value: u64::try_from(value).unwrap_or(u64::MAX),
    })?;

    output.extend_from_slice(&encoded.to_le_bytes());
    Ok(())
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

fn decode_role(value: &str) -> Result<AgentRole, StateError> {
    match value {
        "planner" => Ok(AgentRole::Planner),
        "architect" => Ok(AgentRole::Architect),
        "researcher" => Ok(AgentRole::Researcher),
        "implementer" => Ok(AgentRole::Implementer),
        "tester" => Ok(AgentRole::Tester),
        "reviewer" => Ok(AgentRole::Reviewer),
        "security_reviewer" => Ok(AgentRole::SecurityReviewer),
        "integrator" => Ok(AgentRole::Integrator),
        "release_manager" => Ok(AgentRole::ReleaseManager),
        _ => Err(StateError::InvalidEnum {
            kind: "agent role",
            value: value.to_owned(),
        }),
    }
}

fn decode_capability(value: &str) -> Result<Capability, StateError> {
    match value {
        "read_repository" => Ok(Capability::ReadRepository),
        "write_owned_paths" => Ok(Capability::WriteOwnedPaths),
        "run_local_commands" => Ok(Capability::RunLocalCommands),
        "use_network" => Ok(Capability::UseNetwork),
        "read_github" => Ok(Capability::ReadGitHub),
        "write_github" => Ok(Capability::WriteGitHub),
        "manage_worktrees" => Ok(Capability::ManageWorktrees),
        "read_secrets" => Ok(Capability::ReadSecrets),
        "use_mcp_tools" => Ok(Capability::UseMcpTools),
        "create_pull_request" => Ok(Capability::CreatePullRequest),
        "merge_protected_branch" => Ok(Capability::MergeProtectedBranch),
        "deploy_staging" => Ok(Capability::DeployStaging),
        "deploy_production" => Ok(Capability::DeployProduction),
        _ => Err(StateError::InvalidEnum {
            kind: "capability",
            value: value.to_owned(),
        }),
    }
}

fn decode_approval(value: &str) -> Result<ApprovalBoundary, StateError> {
    match value {
        "activate_implementation_plan" => Ok(ApprovalBoundary::ActivateImplementationPlan),
        "expand_task_scope" => Ok(ApprovalBoundary::ExpandTaskScope),
        "change_dependencies" => Ok(ApprovalBoundary::ChangeDependencies),
        "elevate_capability" => Ok(ApprovalBoundary::ElevateCapability),
        "access_secrets" => Ok(ApprovalBoundary::AccessSecrets),
        "destructive_data_migration" => Ok(ApprovalBoundary::DestructiveDataMigration),
        "irreversible_external_change" => Ok(ApprovalBoundary::IrreversibleExternalChange),
        "merge_protected_branch" => Ok(ApprovalBoundary::MergeProtectedBranch),
        "publish_release" => Ok(ApprovalBoundary::PublishRelease),
        "deploy_production" => Ok(ApprovalBoundary::DeployProduction),
        "change_governance_rules" => Ok(ApprovalBoundary::ChangeGovernanceRules),
        _ => Err(StateError::InvalidEnum {
            kind: "approval boundary",
            value: value.to_owned(),
        }),
    }
}

fn decode_task_state(value: &str) -> Result<TaskState, StateError> {
    match value {
        "pending" => Ok(TaskState::Pending),
        "ready" => Ok(TaskState::Ready),
        "running" => Ok(TaskState::Running),
        "blocked" => Ok(TaskState::Blocked),
        "completed" => Ok(TaskState::Completed),
        "failed" => Ok(TaskState::Failed),
        "cancelled" => Ok(TaskState::Cancelled),
        _ => Err(StateError::InvalidEnum {
            kind: "task state",
            value: value.to_owned(),
        }),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], StateError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(StateError::Truncated)?;

        if end > self.bytes.len() {
            return Err(StateError::Truncated);
        }

        let value = &self.bytes[self.offset..end];
        self.offset = end;

        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, StateError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, StateError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self) -> Result<u64, StateError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_len(&mut self, kind: &'static str, maximum: usize) -> Result<usize, StateError> {
        let raw = self.read_u32()?;
        let value = usize::try_from(raw).map_err(|_| StateError::InvalidLength {
            kind,
            value: u64::from(raw),
        })?;

        if value > maximum {
            return Err(StateError::InvalidLength {
                kind,
                value: u64::from(raw),
            });
        }

        Ok(value)
    }

    fn read_string(&mut self) -> Result<String, StateError> {
        let length = self.read_len("string", MAX_STRING_LEN)?;
        let bytes = self.take(length)?;

        String::from_utf8(bytes.to_vec()).map_err(|_| StateError::InvalidUtf8)
    }

    fn read_strings(&mut self) -> Result<Vec<String>, StateError> {
        let count = self.read_len("string collection", MAX_COLLECTION_LEN)?;
        let mut values = Vec::with_capacity(count);

        for _ in 0..count {
            values.push(self.read_string()?);
        }

        Ok(values)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_snapshot, encode_snapshot};
    use crate::{StateError, StateSnapshot};
    use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
    use agentforge_core::task_graph::{TaskGraph, TaskRecord, TaskState, derive_task_id};

    fn snapshot() -> StateSnapshot {
        let root_id = derive_task_id("P0-M004", "root").expect("root ID");

        let mut root_task = AgentTask::new(
            root_id.to_string(),
            "P0-M004",
            AgentRole::Architect,
            "Define durable task graph state.",
        );

        root_task.capabilities.push(Capability::ReadRepository);
        root_task
            .required_approvals
            .push(ApprovalBoundary::ActivateImplementationPlan);
        root_task.required_gates.push("full".to_owned());

        let root =
            TaskRecord::from_task("root", root_task, TaskState::Completed).expect("root record");

        let child_id = derive_task_id("P0-M004", "state-store").expect("child ID");

        let mut child_task = AgentTask::new(
            child_id.to_string(),
            "P0-M004",
            AgentRole::Implementer,
            "Implement state persistence.",
        );

        child_task.dependency_task_ids.push(root_id.to_string());
        child_task
            .allowed_paths
            .push("crates/agentforge-state/**".to_owned());
        child_task.capabilities.push(Capability::WriteOwnedPaths);

        let child = TaskRecord::from_task("state-store", child_task, TaskState::Pending)
            .expect("child record");

        let mut graph = TaskGraph::new();
        graph.insert(child).expect("insert child");
        graph.insert(root).expect("insert root");
        graph.validate().expect("valid graph");

        StateSnapshot::new(7, graph).expect("valid snapshot")
    }

    #[test]
    fn non_trivial_snapshot_round_trips() {
        let snapshot = snapshot();
        let encoded = encode_snapshot(&snapshot).expect("encode");
        let decoded = decode_snapshot(&encoded).expect("decode");

        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn encoding_is_deterministic() {
        let snapshot = snapshot();

        let first = encode_snapshot(&snapshot).expect("first encoding");
        let second = encode_snapshot(&snapshot).expect("second encoding");

        assert_eq!(first, second);
    }

    #[test]
    fn unsupported_version_fails() {
        let snapshot = snapshot();
        let mut encoded = encode_snapshot(&snapshot).expect("encode");

        encoded[4..6].copy_from_slice(&2_u16.to_le_bytes());

        assert!(matches!(
            decode_snapshot(&encoded),
            Err(StateError::UnsupportedVersion { found: 2 })
        ));
    }

    #[test]
    fn truncated_snapshot_fails() {
        let snapshot = snapshot();
        let mut encoded = encode_snapshot(&snapshot).expect("encode");
        encoded.pop();

        assert!(matches!(
            decode_snapshot(&encoded),
            Err(StateError::Truncated)
        ));
    }

    #[test]
    fn checksum_corruption_fails_closed() {
        let snapshot = snapshot();
        let mut encoded = encode_snapshot(&snapshot).expect("encode");

        let last = encoded.last_mut().expect("encoded body");
        *last ^= 0x01;

        assert!(matches!(
            decode_snapshot(&encoded),
            Err(StateError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn trailing_data_is_rejected() {
        let snapshot = snapshot();
        let mut encoded = encode_snapshot(&snapshot).expect("encode");
        encoded.push(0);

        assert!(matches!(
            decode_snapshot(&encoded),
            Err(StateError::TrailingData)
        ));
    }
}
