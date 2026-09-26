//! Project-local blueprint and guideline intake for AgentForge.
//!
//! Intake documents are deliberately small, versioned, and human-editable. They describe project
//! context and explicit defaults; they never grant authority through prose. Task contracts remain
//! the executable authority boundary owned by `agentforge-core`.

use agentforge_core::agent::{AgentRole, AgentTask, ApprovalBoundary, Capability};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Current supported project-blueprint document version.
pub const BLUEPRINT_VERSION: u16 = 1;
/// Current supported guideline document version.
pub const GUIDELINES_VERSION: u16 = 1;
/// Project-relative blueprint path.
pub const BLUEPRINT_RELATIVE_PATH: &str = ".forge/blueprint.conf";
/// Project-relative guideline path.
pub const GUIDELINES_RELATIVE_PATH: &str = ".forge/guidelines.md";
/// Project-relative gate profile directory; matches `agentforge_gate::GATE_PROFILE_RELATIVE_PATH`.
const GATE_PROFILE_RELATIVE_PATH: &str = ".forge/gates";
const MAX_BLUEPRINT_BYTES: usize = 64 * 1024;
const MAX_GUIDELINE_BYTES: usize = 256 * 1024;
const MAX_FIELD_BYTES: usize = 4096;
const MAX_LIST_ITEMS: usize = 128;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A validated project blueprint with explicit structured defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectBlueprint {
    /// Blueprint schema version.
    pub version: u16,
    /// Human-readable project name.
    pub name: String,
    /// Project mission statement.
    pub mission: String,
    /// Default milestone for newly created tasks, when supplied.
    pub default_milestone: Option<String>,
    /// Default owned paths for newly created tasks.
    pub allowed_paths: Vec<String>,
    /// Default forbidden paths for newly created tasks.
    pub forbidden_paths: Vec<String>,
    /// Explicit default capability grants.
    pub capabilities: Vec<Capability>,
    /// Explicit default approval boundaries.
    pub approvals: Vec<ApprovalBoundary>,
    /// Default quality gates.
    pub gates: Vec<String>,
}

/// A validated guideline document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuidelineDocument {
    /// Guideline schema version.
    pub version: u16,
    /// Markdown guideline body after the version marker.
    pub body: String,
}

/// The validated project-local intake bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeBundle {
    /// Structured project blueprint.
    pub blueprint: ProjectBlueprint,
    /// Human-readable guidelines.
    pub guidelines: GuidelineDocument,
}

/// File contents captured before a guided edit is presented to an operator.
///
/// The snapshot is compared immediately before commit so an unrelated editor cannot be
/// overwritten after the preview was shown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeSnapshot {
    /// Bytes of the blueprint, or `None` when it did not exist.
    pub blueprint: Option<Vec<u8>>,
    /// Bytes of the guidelines, or `None` when they did not exist.
    pub guidelines: Option<Vec<u8>>,
}

/// Explicit task fields supplied by an operator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDraft {
    /// Stable task identifier.
    pub task_id: String,
    /// Milestone identifier owning the task.
    pub milestone_id: Option<String>,
    /// Primary task role.
    pub primary_role: AgentRole,
    /// Positive task goal.
    pub goal: String,
    /// Explicit task exclusions.
    pub non_goals: Vec<String>,
    /// Task dependencies.
    pub dependency_task_ids: Vec<String>,
    /// Task-owned paths; empty uses the blueprint default.
    pub allowed_paths: Vec<String>,
    /// Task-forbidden paths; empty uses the blueprint default.
    pub forbidden_paths: Vec<String>,
    /// Capability grants; empty uses the blueprint default.
    pub capabilities: Vec<Capability>,
    /// Required approvals; empty uses the blueprint default.
    pub required_approvals: Vec<ApprovalBoundary>,
    /// Required gates; empty uses the blueprint default.
    pub required_gates: Vec<String>,
    /// Expected task outputs.
    pub expected_outputs: Vec<String>,
    /// Required evidence.
    pub evidence_requirements: Vec<String>,
}

impl TaskDraft {
    /// Creates a minimal draft with explicit identity, role, and goal.
    #[must_use]
    pub fn new(
        task_id: impl Into<String>,
        primary_role: AgentRole,
        goal: impl Into<String>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            milestone_id: None,
            primary_role,
            goal: goal.into(),
            non_goals: Vec::new(),
            dependency_task_ids: Vec::new(),
            allowed_paths: Vec::new(),
            forbidden_paths: Vec::new(),
            capabilities: Vec::new(),
            required_approvals: Vec::new(),
            required_gates: Vec::new(),
            expected_outputs: Vec::new(),
            evidence_requirements: Vec::new(),
        }
    }
}

/// A source-located validation diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    /// Source file label.
    pub file: String,
    /// One-based source line, or zero for document-level errors.
    pub line: usize,
    /// Stable human-readable diagnostic.
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(formatter, "{}: {}", self.file, self.message)
        } else {
            write!(formatter, "{}:{}: {}", self.file, self.line, self.message)
        }
    }
}

/// Intake operation failure.
#[derive(Debug)]
pub enum IntakeError {
    /// Filesystem failure.
    Io(std::io::Error),
    /// One or more deterministic validation diagnostics.
    Validation(Vec<ValidationError>),
    /// A task contract could not be built from explicit input.
    Task(String),
    /// An initialization target already exists.
    AlreadyExists(PathBuf),
    /// A guided edit observed a newer source than the preview used.
    Conflict(PathBuf),
}

impl fmt::Display for IntakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "intake I/O error: {error}"),
            Self::Validation(errors) => {
                write!(
                    formatter,
                    "intake validation failed ({} error(s))",
                    errors.len()
                )
            }
            Self::Task(error) => write!(formatter, "task creation failed: {error}"),
            Self::AlreadyExists(path) => {
                write!(
                    formatter,
                    "refusing to overwrite existing file: {}",
                    path.display()
                )
            }
            Self::Conflict(path) => write!(
                formatter,
                "guided intake source changed during edit: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for IntakeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Validation(_) | Self::Task(_) | Self::AlreadyExists(_) | Self::Conflict(_) => {
                None
            }
        }
    }
}

impl From<std::io::Error> for IntakeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Returns the project-local blueprint and guideline paths.
#[must_use]
pub fn project_paths(root: impl AsRef<Path>) -> (PathBuf, PathBuf) {
    (
        root.as_ref().join(BLUEPRINT_RELATIVE_PATH),
        root.as_ref().join(GUIDELINES_RELATIVE_PATH),
    )
}

/// Initializes starter intake files without overwriting existing files.
pub fn initialize(root: impl AsRef<Path>) -> Result<(PathBuf, PathBuf), IntakeError> {
    let (blueprint_path, guidelines_path) = project_paths(root);
    if blueprint_path.exists() {
        return Err(IntakeError::AlreadyExists(blueprint_path));
    }
    if guidelines_path.exists() {
        return Err(IntakeError::AlreadyExists(guidelines_path));
    }

    if let Some(parent) = blueprint_path.parent() {
        fs::create_dir_all(parent)?;
    }
    create_new_file(&blueprint_path, starter_blueprint().as_bytes())?;
    if let Err(error) = create_new_file(&guidelines_path, starter_guidelines().as_bytes()) {
        let _ = fs::remove_file(&blueprint_path);
        return Err(error);
    }
    Ok((blueprint_path, guidelines_path))
}

/// Loads and validates both project-local intake documents.
pub fn load(root: impl AsRef<Path>) -> Result<IntakeBundle, IntakeError> {
    let (blueprint_path, guidelines_path) = project_paths(root);
    let blueprint_bytes = fs::read(&blueprint_path)?;
    let guideline_bytes = fs::read(&guidelines_path)?;
    let blueprint = parse_blueprint(&blueprint_path.display().to_string(), &blueprint_bytes)?;
    let guidelines = parse_guidelines(&guidelines_path.display().to_string(), &guideline_bytes)?;
    Ok(IntakeBundle {
        blueprint,
        guidelines,
    })
}

/// Returns the starter values used by a guided intake on an empty project.
#[must_use]
pub fn starter_bundle() -> IntakeBundle {
    IntakeBundle {
        blueprint: ProjectBlueprint {
            version: BLUEPRINT_VERSION,
            name: "My Project".to_owned(),
            mission: "Describe the project outcome.".to_owned(),
            default_milestone: Some("P1-M003".to_owned()),
            allowed_paths: vec!["src".to_owned()],
            forbidden_paths: vec![".env".to_owned()],
            capabilities: vec![Capability::ReadRepository, Capability::WriteOwnedPaths],
            approvals: Vec::new(),
            gates: vec!["full".to_owned()],
        },
        guidelines: GuidelineDocument {
            version: GUIDELINES_VERSION,
            body: "- Keep changes within the task contract.\n- Record evidence for every consequential boundary.\n- Ask for approval when authority is missing.".to_owned(),
        },
    }
}

/// Captures the current intake files without requiring either file to exist.
pub fn snapshot(root: impl AsRef<Path>) -> Result<IntakeSnapshot, IntakeError> {
    let (blueprint, guidelines) = project_paths(root);
    Ok(IntakeSnapshot {
        blueprint: read_optional(&blueprint)?,
        guidelines: read_optional(&guidelines)?,
    })
}

/// Serializes a validated blueprint using the canonical line-oriented format.
pub fn serialize_blueprint(blueprint: &ProjectBlueprint) -> Result<Vec<u8>, IntakeError> {
    let mut output = String::new();
    output.push_str("# AgentForge Project Blueprint v1\n");
    push_scalar(&mut output, "version", &blueprint.version.to_string())?;
    push_scalar(&mut output, "name", &blueprint.name)?;
    push_scalar(&mut output, "mission", &blueprint.mission)?;
    if let Some(value) = &blueprint.default_milestone {
        push_scalar(&mut output, "default_milestone", value)?;
    }
    push_list(&mut output, "allowed_path", &blueprint.allowed_paths)?;
    push_list(&mut output, "forbidden_path", &blueprint.forbidden_paths)?;
    for capability in &blueprint.capabilities {
        push_scalar(&mut output, "capability", capability.as_str())?;
    }
    for approval in &blueprint.approvals {
        push_scalar(&mut output, "approval", approval.as_str())?;
    }
    push_list(&mut output, "gate", &blueprint.gates)?;
    let bytes = output.into_bytes();
    parse_blueprint("<guided blueprint>", &bytes)?;
    Ok(bytes)
}

/// Serializes validated guidelines using the canonical version marker.
pub fn serialize_guidelines(guidelines: &GuidelineDocument) -> Result<Vec<u8>, IntakeError> {
    if guidelines.version != GUIDELINES_VERSION {
        return Err(validation("<guided guidelines>", 0, "version must be 1"));
    }
    if guidelines.body.trim().is_empty() {
        return Err(validation(
            "<guided guidelines>",
            2,
            "guideline body must not be empty",
        ));
    }
    if guidelines.body.len() > MAX_GUIDELINE_BYTES {
        return Err(validation(
            "<guided guidelines>",
            0,
            "guidelines exceed 256 KiB",
        ));
    }
    if guidelines.body.contains('\0') {
        return Err(validation(
            "<guided guidelines>",
            0,
            "guidelines must not contain NUL bytes",
        ));
    }
    let prefix = if guidelines.body.starts_with('\n') {
        "# AgentForge Guidelines v1"
    } else {
        "# AgentForge Guidelines v1\n"
    };
    let bytes = format!("{prefix}{}\n", guidelines.body).into_bytes();
    parse_guidelines("<guided guidelines>", &bytes)?;
    Ok(bytes)
}

/// Atomically writes both intake documents if their pre-edit snapshot is still current.
pub fn commit_documents(
    root: impl AsRef<Path>,
    blueprint: &ProjectBlueprint,
    guidelines: &GuidelineDocument,
    expected: &IntakeSnapshot,
) -> Result<(PathBuf, PathBuf), IntakeError> {
    let root = root.as_ref();
    let current = snapshot(root)?;
    if &current != expected {
        let (blueprint_path, guidelines_path) = project_paths(root);
        let changed = if current.blueprint != expected.blueprint {
            blueprint_path
        } else {
            guidelines_path
        };
        return Err(IntakeError::Conflict(changed));
    }
    let blueprint_bytes = serialize_blueprint(blueprint)?;
    let guidelines_bytes = serialize_guidelines(guidelines)?;
    let (blueprint_path, guidelines_path) = project_paths(root);
    if let Some(parent) = blueprint_path.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_replace(&blueprint_path, &blueprint_bytes)?;
    if let Err(error) = atomic_replace(&guidelines_path, &guidelines_bytes) {
        let _ = restore_file(&blueprint_path, expected.blueprint.as_deref());
        return Err(error);
    }
    Ok((blueprint_path, guidelines_path))
}

/// Restores the exact pre-edit intake files after a later coordinated write fails.
pub fn restore_documents(
    root: impl AsRef<Path>,
    snapshot: &IntakeSnapshot,
) -> Result<(), IntakeError> {
    let (blueprint_path, guidelines_path) = project_paths(root);
    restore_file(&blueprint_path, snapshot.blueprint.as_deref())?;
    restore_file(&guidelines_path, snapshot.guidelines.as_deref())?;
    Ok(())
}

/// Builds a validated task contract from a draft and structured blueprint defaults.
///
/// Explicit draft gates are kept as given. Blueprint default gates are copied only when a matching
/// `.forge/gates/<name>.conf` profile exists under `project_root`; unconfigured defaults are
/// dropped so they cannot fail launch preflight for a task that never asked for them.
pub fn build_task(
    draft: &TaskDraft,
    blueprint: &ProjectBlueprint,
    project_root: &Path,
) -> Result<AgentTask, IntakeError> {
    let milestone_id = draft
        .milestone_id
        .clone()
        .or_else(|| blueprint.default_milestone.clone())
        .ok_or_else(|| {
            IntakeError::Task(
                "milestone is required (use --milestone or set default_milestone)".to_owned(),
            )
        })?;
    let mut task = AgentTask::new(
        draft.task_id.clone(),
        milestone_id,
        draft.primary_role,
        draft.goal.clone(),
    );
    task.non_goals = draft.non_goals.clone();
    task.dependency_task_ids = draft.dependency_task_ids.clone();
    task.allowed_paths = choose(&draft.allowed_paths, &blueprint.allowed_paths);
    task.forbidden_paths = choose(&draft.forbidden_paths, &blueprint.forbidden_paths);
    task.capabilities = choose(&draft.capabilities, &blueprint.capabilities);
    task.required_approvals = choose(&draft.required_approvals, &blueprint.approvals);
    task.required_gates = if draft.required_gates.is_empty() {
        configured_gates(project_root, &blueprint.gates)
    } else {
        draft.required_gates.clone()
    };
    task.expected_outputs = draft.expected_outputs.clone();
    task.evidence_requirements = draft.evidence_requirements.clone();
    task.validate()
        .map_err(|error| IntakeError::Task(error.to_string()))?;
    // An approval the task can never use is a mistake best caught now (P2-M035, finding 21).
    for approval in &task.required_approvals {
        if let Some(capability) = approval.exercised_by() {
            if !task.has_capability(capability) {
                return Err(IntakeError::Task(format!(
                    "approval {} needs capability {}, or the task can be approved but never \
                     carried out; add --capability {}",
                    approval.as_str(),
                    capability.as_str(),
                    capability.as_str()
                )));
            }
        }
    }
    Ok(task)
}

/// Returns the default gates that have a regular gate profile file under `project_root`.
fn configured_gates(project_root: &Path, defaults: &[String]) -> Vec<String> {
    let directory = project_root.join(GATE_PROFILE_RELATIVE_PATH);
    defaults
        .iter()
        .filter(|name| {
            // Only plain gate IDs can name a profile; anything else cannot escape the directory.
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                && fs::symlink_metadata(directory.join(format!("{name}.conf")))
                    .is_ok_and(|metadata| metadata.is_file())
        })
        .cloned()
        .collect()
}

fn choose<T: Clone>(specific: &[T], defaults: &[T]) -> Vec<T> {
    if specific.is_empty() {
        defaults.to_vec()
    } else {
        specific.to_vec()
    }
}

fn create_new_file(path: &Path, bytes: &[u8]) -> Result<(), IntakeError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                IntakeError::AlreadyExists(path.to_owned())
            } else {
                IntakeError::Io(error)
            }
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, IntakeError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(IntakeError::Io(error)),
    }
}

fn push_scalar(output: &mut String, key: &str, value: &str) -> Result<(), IntakeError> {
    if value.is_empty() {
        return Err(IntakeError::Task(format!("{key} must not be empty")));
    }
    if value.len() > MAX_FIELD_BYTES {
        return Err(IntakeError::Task(format!("{key} exceeds 4096 bytes")));
    }
    if value.contains(['\n', '\r', '\0']) {
        return Err(IntakeError::Task(format!(
            "{key} contains an unsupported control byte"
        )));
    }
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
    Ok(())
}

fn push_list(output: &mut String, key: &str, values: &[String]) -> Result<(), IntakeError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(IntakeError::Task(format!("{key} list exceeds 128 items")));
    }
    for value in values {
        push_scalar(output, key, value)?;
    }
    Ok(())
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), IntakeError> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("intake");
    let temporary = path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ));
    let result = (|| -> Result<(), IntakeError> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match fs::rename(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(path)?;
                fs::rename(&temporary, path)?;
                Ok(())
            }
            Err(error) => Err(IntakeError::Io(error)),
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn restore_file(path: &Path, bytes: Option<&[u8]>) -> Result<(), IntakeError> {
    match bytes {
        Some(bytes) => atomic_replace(path, bytes),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(IntakeError::Io(error)),
        },
    }
}

fn parse_blueprint(file: &str, bytes: &[u8]) -> Result<ProjectBlueprint, IntakeError> {
    if bytes.len() > MAX_BLUEPRINT_BYTES {
        return Err(validation(file, 0, "blueprint exceeds 64 KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| validation(file, 0, "blueprint must be valid UTF-8"))?;
    let mut version = None;
    let mut name = None;
    let mut mission = None;
    let mut default_milestone = None;
    let mut allowed_paths = Vec::new();
    let mut forbidden_paths = Vec::new();
    let mut capabilities: Vec<Capability> = Vec::new();
    let mut approvals: Vec<ApprovalBoundary> = Vec::new();
    let mut gates = Vec::new();
    let mut errors = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let value = raw.trim();
        if value.is_empty() || value.starts_with('#') {
            continue;
        }
        let Some((key, raw_value)) = value.split_once('=') else {
            errors.push(diag(file, line, "expected key=value"));
            continue;
        };
        let key = key.trim();
        let value = raw_value.trim();
        if value.len() > MAX_FIELD_BYTES {
            errors.push(diag(file, line, "field exceeds 4096 bytes"));
            continue;
        }
        if value.is_empty() && !matches!(key, "default_milestone") {
            errors.push(diag(file, line, "value must not be empty"));
            continue;
        }
        match key {
            "version" => set_once(
                &mut version,
                value.parse().ok(),
                file,
                line,
                "version",
                &mut errors,
            ),
            "name" => set_once_string(&mut name, value, file, line, "name", &mut errors),
            "mission" => set_once_string(&mut mission, value, file, line, "mission", &mut errors),
            "default_milestone" => set_once_optional(
                &mut default_milestone,
                value,
                file,
                line,
                "default_milestone",
                &mut errors,
            ),
            "allowed_path" => push_bounded(&mut allowed_paths, value, file, line, &mut errors),
            "forbidden_path" => push_bounded(&mut forbidden_paths, value, file, line, &mut errors),
            "capability" => match capability_from_name(value) {
                Some(item) => push_bounded(&mut capabilities, item, file, line, &mut errors),
                None => errors.push(diag(file, line, "unknown capability")),
            },
            "approval" => match approval_from_name(value) {
                Some(item) => push_bounded(&mut approvals, item, file, line, &mut errors),
                None => errors.push(diag(file, line, "unknown approval boundary")),
            },
            "gate" => push_bounded(&mut gates, value, file, line, &mut errors),
            _ => errors.push(diag(file, line, "unknown blueprint key")),
        }
    }
    if version != Some(BLUEPRINT_VERSION) {
        errors.push(diag(file, 0, "version must be 1"));
    }
    if name.is_none() {
        errors.push(diag(file, 0, "name is required"));
    }
    if mission.is_none() {
        errors.push(diag(file, 0, "mission is required"));
    }
    for path in &allowed_paths {
        if forbidden_paths.contains(path) {
            errors.push(diag(
                file,
                0,
                &format!("path is both allowed and forbidden: {path}"),
            ));
        }
    }
    let mut seen_capabilities = Vec::new();
    for capability in &capabilities {
        if seen_capabilities.contains(capability) {
            errors.push(diag(
                file,
                0,
                &format!("capability granted more than once: {capability}"),
            ));
        } else {
            seen_capabilities.push(*capability);
        }
    }
    let mut seen_approvals = Vec::new();
    for approval in &approvals {
        if seen_approvals.contains(approval) {
            errors.push(diag(
                file,
                0,
                &format!("approval declared more than once: {}", approval.as_str()),
            ));
        } else {
            seen_approvals.push(*approval);
        }
    }
    if !errors.is_empty() {
        return Err(IntakeError::Validation(errors));
    }
    Ok(ProjectBlueprint {
        version: BLUEPRINT_VERSION,
        name: name.expect("validated name"),
        mission: mission.expect("validated mission"),
        default_milestone,
        allowed_paths,
        forbidden_paths,
        capabilities,
        approvals,
        gates,
    })
}

fn parse_guidelines(file: &str, bytes: &[u8]) -> Result<GuidelineDocument, IntakeError> {
    if bytes.len() > MAX_GUIDELINE_BYTES {
        return Err(validation(file, 0, "guidelines exceed 256 KiB"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| validation(file, 0, "guidelines must be valid UTF-8"))?;
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Err(validation(
            file,
            1,
            "guidelines require '# AgentForge Guidelines v1'",
        ));
    };
    if header.trim() != "# AgentForge Guidelines v1" {
        return Err(validation(
            file,
            1,
            "guidelines require '# AgentForge Guidelines v1'",
        ));
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    if body.trim().is_empty() {
        return Err(validation(file, 2, "guideline body must not be empty"));
    }
    Ok(GuidelineDocument {
        version: GUIDELINES_VERSION,
        body,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: Option<T>,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else if let Some(value) = value {
        *slot = Some(value);
    } else {
        errors.push(diag(file, line, &format!("invalid {key}")));
    }
}

fn set_once_string(
    slot: &mut Option<String>,
    value: &str,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else {
        *slot = Some(value.to_owned());
    }
}

fn set_once_optional(
    slot: &mut Option<String>,
    value: &str,
    file: &str,
    line: usize,
    key: &str,
    errors: &mut Vec<ValidationError>,
) {
    if slot.is_some() {
        errors.push(diag(file, line, &format!("duplicate {key}")));
    } else if !value.is_empty() {
        *slot = Some(value.to_owned());
    }
}

fn push_bounded<T>(
    items: &mut Vec<T>,
    value: impl Into<T>,
    file: &str,
    line: usize,
    errors: &mut Vec<ValidationError>,
) {
    if items.len() >= MAX_LIST_ITEMS {
        errors.push(diag(file, line, "list exceeds 128 items"));
    } else {
        items.push(value.into());
    }
}

fn validation(file: &str, line: usize, message: &str) -> IntakeError {
    IntakeError::Validation(vec![diag(file, line, message)])
}

fn diag(file: &str, line: usize, message: &str) -> ValidationError {
    ValidationError {
        file: file.to_owned(),
        line,
        message: message.to_owned(),
    }
}

/// Parses a stable capability name from a blueprint or CLI value.
#[must_use]
pub fn capability_from_name(value: &str) -> Option<Capability> {
    Some(match value {
        "read_repository" => Capability::ReadRepository,
        "write_owned_paths" => Capability::WriteOwnedPaths,
        "run_local_commands" => Capability::RunLocalCommands,
        "use_network" => Capability::UseNetwork,
        "read_github" => Capability::ReadGitHub,
        "write_github" => Capability::WriteGitHub,
        "manage_worktrees" => Capability::ManageWorktrees,
        "read_secrets" => Capability::ReadSecrets,
        "use_mcp_tools" => Capability::UseMcpTools,
        "create_pull_request" => Capability::CreatePullRequest,
        "merge_protected_branch" => Capability::MergeProtectedBranch,
        "deploy_staging" => Capability::DeployStaging,
        "deploy_production" => Capability::DeployProduction,
        _ => return None,
    })
}

/// Parses a stable approval-boundary name from a blueprint or CLI value.
#[must_use]
pub fn approval_from_name(value: &str) -> Option<ApprovalBoundary> {
    Some(match value {
        "activate_implementation_plan" => ApprovalBoundary::ActivateImplementationPlan,
        "expand_task_scope" => ApprovalBoundary::ExpandTaskScope,
        "change_dependencies" => ApprovalBoundary::ChangeDependencies,
        "elevate_capability" => ApprovalBoundary::ElevateCapability,
        "access_secrets" => ApprovalBoundary::AccessSecrets,
        "destructive_data_migration" => ApprovalBoundary::DestructiveDataMigration,
        "irreversible_external_change" => ApprovalBoundary::IrreversibleExternalChange,
        "merge_protected_branch" => ApprovalBoundary::MergeProtectedBranch,
        "publish_release" => ApprovalBoundary::PublishRelease,
        "deploy_production" => ApprovalBoundary::DeployProduction,
        "change_governance_rules" => ApprovalBoundary::ChangeGovernanceRules,
        _ => return None,
    })
}

/// Parses a stable agent-role name from a CLI value.
#[must_use]
pub fn role_from_name(value: &str) -> Option<AgentRole> {
    Some(match value {
        "planner" => AgentRole::Planner,
        "architect" => AgentRole::Architect,
        "researcher" => AgentRole::Researcher,
        "implementer" => AgentRole::Implementer,
        "tester" => AgentRole::Tester,
        "reviewer" => AgentRole::Reviewer,
        "security_reviewer" => AgentRole::SecurityReviewer,
        "integrator" => AgentRole::Integrator,
        "release_manager" => AgentRole::ReleaseManager,
        _ => return None,
    })
}

fn starter_blueprint() -> String {
    "# AgentForge Project Blueprint v1\nversion=1\nname=My Project\nmission=Describe the project outcome.\ndefault_milestone=P1-M003\nallowed_path=src\nforbidden_path=.env\ncapability=read_repository\ncapability=write_owned_paths\ngate=full\n".to_owned()
}

fn starter_guidelines() -> String {
    "# AgentForge Guidelines v1\n\n- Keep changes within the task contract.\n- Record evidence for every consequential boundary.\n- Ask for approval when authority is missing.\n".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMPORARY_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agentforge-intake-{}-{suffix}-{}",
            std::process::id(),
            TEMPORARY_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("directory");
        path
    }

    #[test]
    fn initialization_is_non_overwriting_and_validates() {
        let root = temporary_root();
        let (blueprint, guidelines) = initialize(&root).expect("initialize");
        let bundle = load(&root).expect("load");
        assert_eq!(bundle.blueprint.version, BLUEPRINT_VERSION);
        assert_eq!(bundle.guidelines.version, GUIDELINES_VERSION);
        assert!(matches!(
            initialize(&root),
            Err(IntakeError::AlreadyExists(_))
        ));
        assert!(blueprint.exists());
        assert!(guidelines.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn malformed_blueprint_reports_source_location() {
        let error =
            parse_blueprint("blueprint.conf", b"version=1\nunknown=x\n").expect_err("invalid");
        assert!(
            matches!(error, IntakeError::Validation(errors) if errors.iter().any(|e| e.line == 2))
        );
    }

    #[test]
    fn task_uses_only_structured_defaults() {
        let blueprint = ProjectBlueprint {
            version: 1,
            name: "x".into(),
            mission: "y".into(),
            default_milestone: Some("P1-M003".into()),
            allowed_paths: vec!["src".into()],
            forbidden_paths: vec![".env".into()],
            capabilities: vec![Capability::ReadRepository],
            approvals: Vec::new(),
            gates: vec!["full".into()],
        };
        let root = temporary_root();
        let gate_dir = root.join(GATE_PROFILE_RELATIVE_PATH);
        fs::create_dir_all(&gate_dir).expect("gate directory");
        fs::write(gate_dir.join("full.conf"), "version=1\n").expect("gate profile");
        let task = build_task(
            &TaskDraft::new("T1", AgentRole::Implementer, "edit"),
            &blueprint,
            &root,
        )
        .expect("task");
        assert_eq!(task.allowed_paths, vec!["src"]);
        assert_eq!(task.capabilities, vec![Capability::ReadRepository]);
        assert_eq!(task.required_gates, vec!["full"]);
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn gate_blueprint(gates: &[&str]) -> ProjectBlueprint {
        ProjectBlueprint {
            version: 1,
            name: "x".into(),
            mission: "y".into(),
            default_milestone: Some("P1-M007".into()),
            allowed_paths: vec!["src".into()],
            forbidden_paths: Vec::new(),
            capabilities: vec![Capability::ReadRepository],
            approvals: Vec::new(),
            gates: gates.iter().map(|gate| (*gate).to_owned()).collect(),
        }
    }

    #[test]
    fn approvals_need_the_capability_that_exercises_them() {
        let root = temporary_root();
        for (approval, capability) in [
            (
                ApprovalBoundary::MergeProtectedBranch,
                Capability::MergeProtectedBranch,
            ),
            (
                ApprovalBoundary::DeployProduction,
                Capability::DeployProduction,
            ),
        ] {
            let mut draft = TaskDraft::new("T1", AgentRole::Implementer, "edit");
            draft.required_approvals = vec![approval];
            draft.capabilities = vec![Capability::ReadRepository];
            let error = build_task(&draft, &gate_blueprint(&[]), &root).expect_err("refused");
            assert!(
                error
                    .to_string()
                    .contains(&format!("add --capability {}", capability.as_str())),
                "{error}"
            );
            draft.capabilities.push(capability);
            assert!(build_task(&draft, &gate_blueprint(&[]), &root).is_ok());
        }
        // Approvals with no exercising capability are unaffected.
        let mut draft = TaskDraft::new("T1", AgentRole::Implementer, "edit");
        draft.required_approvals = vec![
            ApprovalBoundary::PublishRelease,
            ApprovalBoundary::ActivateImplementationPlan,
        ];
        assert!(build_task(&draft, &gate_blueprint(&[]), &root).is_ok());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn blueprint_default_gates_are_kept_only_when_configured() {
        let root = temporary_root();
        let gate_dir = root.join(GATE_PROFILE_RELATIVE_PATH);
        fs::create_dir_all(&gate_dir).expect("gate directory");
        fs::write(gate_dir.join("workspace.conf"), "version=1\n").expect("gate profile");
        let task = build_task(
            &TaskDraft::new("T1", AgentRole::Implementer, "edit"),
            &gate_blueprint(&["full", "workspace", "../escape"]),
            &root,
        )
        .expect("task");
        assert_eq!(
            task.required_gates,
            vec!["workspace"],
            "unconfigured defaults are dropped"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn blueprint_default_gates_are_dropped_without_profiles() {
        let root = temporary_root();
        let task = build_task(
            &TaskDraft::new("T1", AgentRole::Implementer, "edit"),
            &gate_blueprint(&["full"]),
            &root,
        )
        .expect("task");
        assert!(task.required_gates.is_empty());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn explicit_gates_are_kept_even_when_unconfigured() {
        let root = temporary_root();
        let mut draft = TaskDraft::new("T1", AgentRole::Implementer, "edit");
        draft.required_gates = vec!["missing".into()];
        let task = build_task(&draft, &gate_blueprint(&["full"]), &root).expect("task");
        assert_eq!(task.required_gates, vec!["missing"]);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn guided_serialization_round_trips_and_is_deterministic() {
        let bundle = starter_bundle();
        let first = serialize_blueprint(&bundle.blueprint).expect("blueprint");
        let second = serialize_blueprint(&bundle.blueprint).expect("blueprint");
        assert_eq!(first, second);
        assert_eq!(
            parse_blueprint("guided", &first).expect("round trip"),
            bundle.blueprint
        );
        let guidelines = serialize_guidelines(&bundle.guidelines).expect("guidelines");
        assert_eq!(
            parse_guidelines("guided", &guidelines).expect("round trip"),
            bundle.guidelines
        );
    }

    #[test]
    fn guided_commit_rejects_a_changed_source() {
        let root = temporary_root();
        initialize(&root).expect("initialize");
        let expected = snapshot(&root).expect("snapshot");
        let bundle = load(&root).expect("load");
        fs::write(
            root.join(GUIDELINES_RELATIVE_PATH),
            "# AgentForge Guidelines v1\n\nchanged\n",
        )
        .expect("external edit");
        assert!(matches!(
            commit_documents(&root, &bundle.blueprint, &bundle.guidelines, &expected),
            Err(IntakeError::Conflict(path)) if path.ends_with(GUIDELINES_RELATIVE_PATH)
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
