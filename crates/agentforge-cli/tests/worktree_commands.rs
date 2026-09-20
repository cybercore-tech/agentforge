#![allow(missing_docs)]

mod worktree_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn worktree_lifecycle_commands_are_deterministic_and_safe() {
        let root = temporary_repo();
        let task_id = "P2-M006-T0001";
        let created = forge(
            &root,
            &[
                "worktree",
                "create",
                root.to_str().unwrap(),
                task_id,
                "HEAD",
            ],
        );
        assert!(created.status.success(), "{created:?}");
        let created_stdout = String::from_utf8_lossy(&created.stdout);
        assert!(created_stdout.contains("created worktree task=P2-M006-T0001"));
        assert!(created_stdout.contains("branch=agentforge/task/P2-M006-T0001"));
        assert!(created_stdout.contains("dirty=false operation=none"));

        let inspected = forge(
            &root,
            &["worktree", "inspect", root.to_str().unwrap(), task_id],
        );
        assert!(inspected.status.success(), "{inspected:?}");
        assert!(String::from_utf8_lossy(&inspected.stdout).contains("inspected worktree"));

        let listed = forge(&root, &["worktree", "list", root.to_str().unwrap()]);
        assert!(listed.status.success(), "{listed:?}");
        assert!(String::from_utf8_lossy(&listed.stdout).contains(task_id));

        let path = root.join(".forge/worktrees").join(task_id);
        fs::write(path.join("untracked.txt"), "dirty\n").unwrap();
        let refused = forge(
            &root,
            &["worktree", "retire", root.to_str().unwrap(), task_id],
        );
        assert!(!refused.status.success(), "{refused:?}");
        assert!(String::from_utf8_lossy(&refused.stderr).contains("dirty"));
        fs::remove_file(path.join("untracked.txt")).unwrap();

        let retired = forge(
            &root,
            &["worktree", "retire", root.to_str().unwrap(), task_id],
        );
        assert!(retired.status.success(), "{retired:?}");
        assert!(String::from_utf8_lossy(&retired.stdout).contains("branch-preserved=true"));
        let branch = git_output(
            &root,
            &[
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/agentforge/task/P2-M006-T0001",
            ],
        );
        assert!(branch.status.success(), "{branch:?}");
        fs::remove_dir_all(root).unwrap();
    }

    fn forge(root: &Path, arguments: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_forge"))
            .args(arguments)
            .current_dir(root)
            .output()
            .unwrap()
    }

    fn temporary_repo() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agentforge-cli-worktree-{stamp}"));
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q"]);
        fs::write(root.join("README.md"), "worktree fixture\n").unwrap();
        git(
            &root,
            &["config", "user.email", "agentforge@example.invalid"],
        );
        git(&root, &["config", "user.name", "AgentForge Test"]);
        git(&root, &["add", "README.md"]);
        git(&root, &["commit", "-qm", "fixture"]);
        root
    }

    fn git(root: &Path, arguments: &[&str]) {
        let output = git_output(root, arguments);
        assert!(output.status.success(), "git {:?}: {output:?}", arguments);
    }

    fn git_output(root: &Path, arguments: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(arguments)
            .current_dir(root)
            .output()
            .unwrap()
    }
}
