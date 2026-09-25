//! `scripts/worker-bundle` and `scripts/worker-host-setup` end to end (P4-M011): a worker host set
//! up only by the scripts passes the doctor and runs a remote task.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn scripts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts")
}

fn forge_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// PATH with the freshly built `forge` first, and Git isolated from the user's global config.
fn command(program: impl AsRef<std::ffi::OsStr>, home: &Path) -> Command {
    let mut command = Command::new(program);
    let path = format!(
        "{}:{}",
        forge_path().parent().expect("bin dir").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    command
        .env("PATH", path)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    command
}

fn run(mut command: Command) -> (Output, String) {
    let output = command.output().expect("run");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output, text)
}

fn git(directory: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("git");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

#[test]
fn a_worker_host_set_up_by_the_scripts_runs_a_remote_task() {
    let base = std::env::temp_dir().join(format!("agentforge-worker-setup-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let root = base.join("coordinator");
    let home = base.join("host-home");
    fs::create_dir_all(&root).expect("root");
    fs::create_dir_all(&home).expect("home");
    let root_text = root.to_str().expect("root");

    // A coordinator project with one task, which is also the origin the worker clones.
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "Coordinator"]);
    git(&root, &["config", "user.email", "c@example.invalid"]);
    fs::write(root.join(".gitignore"), ".forge/\n").expect("gitignore");
    git(&root, &["add", ".gitignore"]);
    git(&root, &["commit", "-qm", "init"]);
    let forge = |arguments: &[&str]| {
        run({
            let mut command = command(forge_path(), &home);
            command.args(arguments);
            command
        })
    };
    assert!(forge(&["init", root_text]).0.status.success());
    let (created, text) = forge(&[
        "task",
        "create",
        root_text,
        "P4-M011-T0001",
        "P4-M011",
        "implementer",
        "set up by the scripts",
        "--allowed",
        "agentforge-fixture-output.txt",
        "--capability",
        "run_local_commands",
    ]);
    assert!(created.status.success(), "{text}");
    let server = agentforge_daemon::worker_api::WorkerApiServer::start(
        &root,
        "127.0.0.1:0".parse().expect("addr"),
    )
    .expect("worker api");
    let port = server.address.port().to_string();

    // Coordinator side.
    let bundle = base.join("remote-1.bundle");
    let bundle_command = |api_port: &str| {
        let mut command = command(scripts().join("worker-bundle"), &home);
        command.args([
            root_text,
            "remote-1",
            "--origin",
            root_text,
            "--api-port",
            api_port,
            "--out",
            bundle.to_str().expect("bundle"),
        ]);
        command
    };
    let (bundled, text) = run(bundle_command(&port));
    assert!(bundled.status.success(), "{text}");
    assert!(text.contains("enrolled worker remote-1"), "{text}");
    let secret = fs::read_to_string(root.join(".forge/workers/remote-1.secret")).expect("secret");
    assert!(!text.contains(secret.trim()), "the secret is never printed");
    assert_eq!(mode(&bundle), 0o700);
    assert_eq!(mode(&bundle.join("secret")), 0o600);
    assert_eq!(
        fs::read_to_string(root.join(".forge/worker-api.conf")).expect("api conf"),
        format!("bind=127.0.0.1:{port}\n")
    );
    // Again: reuses everything; a different API port is refused, not applied.
    let (again, text) = run(bundle_command(&port));
    assert!(again.status.success(), "{text}");
    assert!(
        text.contains("already registered") && text.contains("already enrolled"),
        "{text}"
    );
    let (conflict, text) = run(bundle_command("1"));
    assert!(!conflict.status.success());
    assert!(text.contains("already binds something else"), "{text}");

    // Worker-host side, as a rehearsal without GhostPort against the loopback API.
    let clone = base.join("host/src/project");
    let endpoint = format!("127.0.0.1:{port}");
    let setup = |extra: &[&str]| {
        let mut command = command(scripts().join("worker-host-setup"), &home);
        command
            .arg(&bundle)
            .args(["--repo", clone.to_str().expect("clone")])
            .args(["--profile", "fixture"])
            .args([
                "--agent-executable",
                env!("CARGO_BIN_EXE_agentforge-cli-fixture"),
            ])
            .args(["--endpoint", &endpoint, "--no-ghostport"])
            .args(extra);
        run(command)
    };
    let (refused, text) = setup(&[]);
    assert!(!refused.status.success());
    assert!(text.contains("the clone has no user.name"), "{text}");
    let identity = [
        "--git-name",
        "Remote Worker",
        "--git-email",
        "w@example.invalid",
    ];
    let (first, text) = setup(&identity);
    assert!(first.status.success(), "{text}");
    for check in [
        "repo",
        "git-identity",
        "agent",
        "secret",
        "endpoint",
        "origin",
    ] {
        assert!(
            text.contains(&format!("doctor ok {check}:")),
            "{check}:\n{text}"
        );
    }
    assert!(!text.contains(secret.trim()), "the secret is never printed");
    let secret_path = home.join(".config/agentforge/worker-remote-1.secret");
    let env_path = home.join(".config/agentforge/worker-remote-1.env");
    assert_eq!(mode(&secret_path), 0o600);
    assert_eq!(mode(&env_path), 0o600);
    let env = fs::read_to_string(&env_path).expect("env");
    assert!(
        env.contains(&format!("AGENTFORGE_ENDPOINT={endpoint}\n")),
        "{env}"
    );

    // A second run changes nothing and still passes.
    let before = fs::read_to_string(clone.join(".forge/agents/fixture.conf")).expect("profile");
    let (second, text) = setup(&[]);
    assert!(second.status.success(), "{text}");
    assert!(
        !text.contains("wrote:") && !text.contains("cloned"),
        "{text}"
    );
    assert!(
        text.contains("clone exists") && text.contains("unchanged:"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(clone.join(".forge/agents/fixture.conf")).expect("profile"),
        before
    );

    // The worker runs a real remote task from the env file's values alone.
    let (granted, text) = forge(&[
        "lease",
        "grant",
        root_text,
        "P4-M011-T0001",
        "--actor",
        "op",
    ]);
    assert!(granted.status.success(), "{text}");
    let value = |key: &str| {
        env.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .expect("env value")
            .to_owned()
    };
    let (ran, text) = forge(&[
        "worker",
        "remote",
        "run",
        "--endpoint",
        &value("AGENTFORGE_ENDPOINT"),
        "--worker",
        "remote-1",
        "--secret-file",
        &value("AGENTFORGE_SECRET_FILE"),
        "--repo",
        &value("AGENTFORGE_REPO"),
        "--profile",
        &value("AGENTFORGE_PROFILE"),
        "--once",
    ]);
    assert!(ran.status.success(), "{text}");
    assert!(text.contains("claimed=1 imported=1 abandoned=0"), "{text}");
    drop(server);
    fs::remove_dir_all(base).expect("cleanup");
}
