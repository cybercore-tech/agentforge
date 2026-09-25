//! Exact-SHA import of remote results (P4-M008). Unix-only: gate fixtures are `true`/`false`.
#![cfg(unix)]

use agentforge_audit::{AuditEventKind, AuditStore, FileAuditStore};
use agentforge_core::agent::{AgentRole, AgentTask, Capability};
use agentforge_core::remote::{
    LeaseBook, LeaseId, RemoteWorkerDescriptor, RemoteWorkerId, WorkerCapability,
};
use agentforge_core::task::{TaskGraph, TaskId, TaskState};
use agentforge_orchestrator::{LeaseClaim, RemoteResult, import_remote_result, wall_clock_ms};
use agentforge_state::{FileLeaseStore, FileTaskStore, LeaseStore, TaskStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const TASK: &str = "P4-M008-T0001";
const BRANCH: &str = "agentforge/task/P4-M008-T0001";

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

struct Fixture {
    dir: PathBuf,
    coordinator: PathBuf,
    clone: PathBuf,
    base: String,
    store: FileTaskStore,
}

impl Fixture {
    /// A coordinator repo with task T0001 (allowed `src`, forbidden `src/secret`) leased to
    /// worker `w-a`, an optional gate, and a clone standing in for the remote worker host.
    fn new(gate: Option<&str>) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "agentforge-remote-import-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        let coordinator = dir.join("coordinator");
        fs::create_dir_all(&coordinator).expect("dir");
        git(&coordinator, &["init", "-q", "-b", "main"]);
        git(&coordinator, &["config", "user.name", "Coordinator"]);
        git(&coordinator, &["config", "user.email", "c@example.invalid"]);
        fs::write(coordinator.join(".gitignore"), ".forge/\n").expect("ignore");
        fs::write(coordinator.join("README.md"), "base\n").expect("readme");
        git(&coordinator, &["add", "-A"]);
        git(&coordinator, &["commit", "-qm", "base"]);
        let base = git(&coordinator, &["rev-parse", "HEAD"]);

        let mut task = AgentTask::new(TASK, "P4-M008", AgentRole::Implementer, "remote import");
        task.capabilities = vec![Capability::RunLocalCommands];
        task.allowed_paths = vec!["src".into()];
        task.forbidden_paths = vec!["src/secret".into()];
        let store = FileTaskStore::for_project_root(&coordinator);
        store
            .save(&TaskGraph::from_tasks([task]).expect("graph"))
            .expect("snapshot");
        if let Some(executable) = gate {
            let gates = coordinator.join(".forge/gates");
            fs::create_dir_all(&gates).expect("gates");
            fs::write(
                gates.join("check.conf"),
                format!("version=1\nexecutable={executable}\n"),
            )
            .expect("gate");
        }
        let worker = RemoteWorkerDescriptor::new(
            RemoteWorkerId::parse("w-a").expect("worker"),
            "linux-x86_64",
            vec![WorkerCapability::parse("rust").expect("capability")],
            1,
        )
        .expect("descriptor");
        let mut book = LeaseBook::new();
        let now = wall_clock_ms();
        book.grant(
            &worker,
            LeaseId::parse(format!("{TASK}.L1")).expect("lease"),
            TaskId::parse(TASK).expect("task"),
            now,
            now + 600_000,
        )
        .expect("grant");
        FileLeaseStore::for_project_root(&coordinator)
            .save(&book)
            .expect("leases");

        let clone = dir.join("clone");
        git(
            &dir,
            &["clone", "-q", coordinator.to_str().expect("path"), "clone"],
        );
        git(&clone, &["config", "user.name", "Remote"]);
        git(&clone, &["config", "user.email", "r@example.invalid"]);
        git(&clone, &["checkout", "-q", "-b", BRANCH, &base]);
        Self {
            dir,
            coordinator,
            clone,
            base,
            store,
        }
    }

    /// Commits `files` on the clone's task branch and bundles `base..branch`.
    fn remote_commit(&self, files: &[&str]) -> (String, PathBuf) {
        for file in files {
            let path = self.clone.join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
            fs::write(&path, format!("remote work in {file}\n")).expect("write");
        }
        git(&self.clone, &["add", "-A"]);
        git(&self.clone, &["commit", "-qm", "remote work"]);
        let head = git(&self.clone, &["rev-parse", "HEAD"]);
        let bundle = self.dir.join("result.bundle");
        git(
            &self.clone,
            &[
                "bundle",
                "create",
                "-q",
                bundle.to_str().expect("path"),
                &format!("{}..{BRANCH}", self.base),
            ],
        );
        (head, bundle)
    }

    fn result(&self, head: &str, bundle: Option<PathBuf>, generation: u64) -> RemoteResult {
        RemoteResult {
            claim: LeaseClaim {
                lease_id: LeaseId::parse(format!("{TASK}.L1")).expect("lease"),
                worker_id: RemoteWorkerId::parse("w-a").expect("worker"),
                generation,
            },
            base_commit: self.base.clone(),
            head_commit: head.to_owned(),
            bundle,
            exit_code: Some(0),
            termination: "exited".into(),
            stdout: b"remote stdout\n".to_vec(),
            stderr: Vec::new(),
        }
    }

    fn import(
        &self,
        result: &RemoteResult,
    ) -> Result<agentforge_orchestrator::RemoteImport, String> {
        let mut audit =
            FileAuditStore::open(self.coordinator.join(".forge/audit.log")).expect("audit");
        import_remote_result(
            &self.coordinator,
            &self.store,
            &mut audit,
            &TaskId::parse(TASK).expect("task"),
            result,
        )
        .map_err(|error| error.to_string())
    }

    fn state(&self) -> TaskState {
        self.store
            .load()
            .expect("load")
            .expect("graph")
            .get(&TaskId::parse(TASK).expect("task"))
            .expect("record")
            .state()
    }

    /// Nothing was written: task pending, no audit, no task branch, no quarantine ref.
    fn assert_untouched(&self) {
        assert_eq!(self.state(), TaskState::Pending);
        let audit = FileAuditStore::open(self.coordinator.join(".forge/audit.log")).expect("audit");
        assert!(audit.records().is_empty(), "audit written");
        assert_eq!(
            git(
                &self.coordinator,
                &["for-each-ref", "refs/agentforge", "refs/heads/agentforge"]
            ),
            "",
            "refs left behind"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_verified_result_is_imported_at_its_exact_commit_and_gated_locally() {
    let fixture = Fixture::new(Some("/usr/bin/true"));
    let (head, bundle) = fixture.remote_commit(&["src/lib.txt"]);
    let import = fixture
        .import(&fixture.result(&head, Some(bundle), 1))
        .expect("import");
    assert_eq!(import.state, TaskState::Running, "awaiting review");
    assert_eq!(import.gates.len(), 1);
    assert!(import.gates[0].passed());
    let worktree = import.worktree.expect("worktree");
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&fixture.coordinator, &["rev-parse", BRANCH]), head);
    assert_eq!(
        git(&fixture.coordinator, &["for-each-ref", "refs/agentforge"]),
        "",
        "quarantine ref removed"
    );
    let audit = FileAuditStore::open(fixture.coordinator.join(".forge/audit.log")).expect("audit");
    let finished = audit
        .records()
        .iter()
        .map(|record| record.event())
        .find(|event| event.kind() == AuditEventKind::AgentFinished)
        .expect("agent finished");
    assert_eq!(finished.fields()["channel"], "remote");
    assert_eq!(finished.fields()["worker"], "w-a");
    assert_eq!(finished.fields()["head_commit"], head);
    let stdout_log = fixture.coordinator.join(&finished.fields()["stdout_log"]);
    assert_eq!(fs::read(stdout_log).expect("log"), b"remote stdout\n");
    assert!(
        audit
            .records()
            .iter()
            .any(|record| record.event().kind() == AuditEventKind::GateFinished)
    );
}

#[test]
fn a_failing_local_gate_fails_the_imported_task() {
    let fixture = Fixture::new(Some("/usr/bin/false"));
    let (head, bundle) = fixture.remote_commit(&["src/lib.txt"]);
    let import = fixture
        .import(&fixture.result(&head, Some(bundle), 1))
        .expect("import");
    assert_eq!(import.state, TaskState::Failed);
    assert_eq!(fixture.state(), TaskState::Failed);
}

#[test]
fn mismatched_head_wrong_claim_and_corrupt_bundles_change_nothing() {
    let fixture = Fixture::new(None);
    let (head, bundle) = fixture.remote_commit(&["src/lib.txt"]);
    let mut wrong_head = head.clone();
    wrong_head.replace_range(39..40, if head.ends_with('0') { "1" } else { "0" });
    let error = fixture
        .import(&fixture.result(&wrong_head, Some(bundle.clone()), 1))
        .expect_err("wrong head");
    assert!(error.contains("but the worker reported"), "{error}");
    fixture.assert_untouched();

    let error = fixture
        .import(&fixture.result(&head, Some(bundle.clone()), 2))
        .expect_err("wrong generation");
    assert!(error.contains("does not match an active lease"), "{error}");
    fixture.assert_untouched();

    let corrupt = fixture.dir.join("corrupt.bundle");
    fs::write(&corrupt, b"not a bundle").expect("corrupt");
    let error = fixture
        .import(&fixture.result(&head, Some(corrupt), 1))
        .expect_err("corrupt");
    assert!(error.contains("does not verify"), "{error}");
    fixture.assert_untouched();

    let error = fixture
        .import(&fixture.result(&head, None, 1))
        .expect_err("missing bundle");
    assert!(error.contains("no bundle"), "{error}");
    fixture.assert_untouched();

    // The genuine result still imports afterwards.
    fixture
        .import(&fixture.result(&head, Some(bundle), 1))
        .expect("import");
}

#[test]
fn out_of_bounds_paths_are_rejected() {
    for (file, expected) in [
        ("docs/notes.md", "outside the task's allowed paths"),
        ("src/secret/key.txt", "forbidden path"),
    ] {
        let fixture = Fixture::new(None);
        let (head, bundle) = fixture.remote_commit(&["src/ok.txt", file]);
        let error = fixture
            .import(&fixture.result(&head, Some(bundle), 1))
            .expect_err(file);
        assert!(error.contains(expected), "{file}: {error}");
        fixture.assert_untouched();
    }
}

#[test]
fn a_head_that_does_not_descend_from_base_is_rejected() {
    let fixture = Fixture::new(None);
    // An unrelated history on the task branch name.
    git(&fixture.clone, &["checkout", "-q", "--orphan", "unrelated"]);
    git(&fixture.clone, &["rm", "-rq", "--cached", "."]);
    fs::create_dir_all(fixture.clone.join("src")).expect("src");
    fs::write(fixture.clone.join("src/x.txt"), "orphan\n").expect("write");
    git(&fixture.clone, &["add", "src/x.txt"]);
    git(&fixture.clone, &["commit", "-qm", "orphan"]);
    git(&fixture.clone, &["branch", "-f", BRANCH, "HEAD"]);
    let head = git(&fixture.clone, &["rev-parse", "HEAD"]);
    let bundle = fixture.dir.join("orphan.bundle");
    git(
        &fixture.clone,
        &[
            "bundle",
            "create",
            "-q",
            bundle.to_str().expect("path"),
            BRANCH,
        ],
    );
    let error = fixture
        .import(&fixture.result(&head, Some(bundle), 1))
        .expect_err("unrelated");
    assert!(error.contains("does not descend from"), "{error}");
    fixture.assert_untouched();
}

#[test]
fn a_result_without_commits_fails_the_task_with_evidence() {
    let fixture = Fixture::new(None);
    let mut result = fixture.result(&fixture.base, None, 1);
    result.exit_code = Some(3);
    let import = fixture.import(&result).expect("import");
    assert_eq!(import.state, TaskState::Failed);
    assert!(import.worktree.is_none());
    let audit = FileAuditStore::open(fixture.coordinator.join(".forge/audit.log")).expect("audit");
    let reasons = audit
        .records()
        .iter()
        .filter_map(|record| record.event().fields().get("reason").cloned())
        .collect::<Vec<_>>();
    assert_eq!(reasons, ["no changes"]);
}
