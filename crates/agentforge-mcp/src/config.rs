//! External MCP server declarations: `.forge/mcp/<server>.conf` (P3-M006, ADR-0055).
//!
//! ```text
//! version=1
//! executable=/absolute/path            # the server program
//! argument=...                         # repeatable, literal
//! env.NAME=value                       # literal environment (the child environment is cleared)
//! pass_env=NAME                        # copy NAME from the gateway's environment (for secrets)
//! timeout_ms=30000                     # per call; default 30 s, maximum 10 min
//! tool.<upstream-tool>=<capability>    # expose this tool, requiring this capability
//! ```
//!
//! Only tools named by a `tool.` line are ever exposed: unmapped means invisible.

use agentforge_core::agent::Capability;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where a project declares its external MCP servers.
pub const MCP_SERVER_RELATIVE_PATH: &str = ".forge/mcp";
/// Largest server declaration accepted.
pub const MAX_SERVER_CONFIG_BYTES: u64 = 64 * 1024;
/// Per-call timeout when `timeout_ms` is not given.
pub const DEFAULT_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest per-call timeout accepted.
pub const MAX_UPSTREAM_TIMEOUT: Duration = Duration::from_secs(600);

/// One declared external MCP server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    /// Server name (the file stem); exposed tools are named `<name>__<tool>`.
    pub name: String,
    /// Absolute path of the server program.
    pub executable: PathBuf,
    /// Literal arguments.
    pub arguments: Vec<String>,
    /// Literal environment values.
    pub env: BTreeMap<String, String>,
    /// Variables copied from the gateway's own environment.
    pub pass_env: Vec<String>,
    /// Per-call timeout.
    pub timeout: Duration,
    /// Exposed upstream tools and the capability each requires.
    pub tools: BTreeMap<String, Capability>,
}

/// Whether `name` is a valid server name: `[a-z0-9-]{1,32}`, so `__` never appears in it.
#[must_use]
pub fn valid_server_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Whether `name` is a valid MCP tool name: `[A-Za-z0-9_-]{1,64}`.
#[must_use]
pub fn valid_tool_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// Parses a capability by its stable name.
#[must_use]
pub fn capability_named(name: &str) -> Option<Capability> {
    const ALL: [Capability; 13] = [
        Capability::ReadRepository,
        Capability::WriteOwnedPaths,
        Capability::RunLocalCommands,
        Capability::UseNetwork,
        Capability::ReadGitHub,
        Capability::WriteGitHub,
        Capability::ManageWorktrees,
        Capability::ReadSecrets,
        Capability::UseMcpTools,
        Capability::CreatePullRequest,
        Capability::MergeProtectedBranch,
        Capability::DeployStaging,
        Capability::DeployProduction,
    ];
    ALL.into_iter()
        .find(|capability| capability.as_str() == name)
}

/// Parses one declaration. Errors name the line.
pub fn parse_server_config(name: &str, text: &str) -> Result<ServerConfig, String> {
    if !valid_server_name(name) {
        return Err(format!(
            "invalid server name {name:?} (use [a-z0-9-], up to 32)"
        ));
    }
    let mut version = None;
    let mut executable = None;
    let mut arguments = Vec::new();
    let mut env = BTreeMap::new();
    let mut pass_env = Vec::new();
    let mut timeout = None;
    let mut tools = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let at = |reason: String| format!("line {}: {reason}", index + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| at("expected key=value".into()))?;
        let (key, value) = (key.trim(), value.trim());
        match key {
            "version" if version.is_none() => {
                if value != "1" {
                    return Err(at(format!("unsupported version {value}")));
                }
                version = Some(());
            }
            "executable" if executable.is_none() => {
                let path = PathBuf::from(value);
                if !path.is_absolute() {
                    return Err(at("executable must be an absolute path".into()));
                }
                executable = Some(path);
            }
            "argument" => arguments.push(value.to_owned()),
            "pass_env" => {
                if !valid_env_name(value) {
                    return Err(at(format!("invalid variable name {value:?}")));
                }
                pass_env.push(value.to_owned());
            }
            "timeout_ms" if timeout.is_none() => {
                let millis = value
                    .parse::<u64>()
                    .ok()
                    .filter(|millis| *millis > 0)
                    .ok_or_else(|| at(format!("timeout_ms must be a positive number: {value}")))?;
                let duration = Duration::from_millis(millis);
                if duration > MAX_UPSTREAM_TIMEOUT {
                    return Err(at("timeout_ms exceeds 600000".into()));
                }
                timeout = Some(duration);
            }
            key if key.starts_with("env.") => {
                let variable = &key["env.".len()..];
                if !valid_env_name(variable) {
                    return Err(at(format!("invalid variable name {variable:?}")));
                }
                env.insert(variable.to_owned(), value.to_owned());
            }
            key if key.starts_with("tool.") => {
                let tool = &key["tool.".len()..];
                if !valid_tool_name(tool) {
                    return Err(at(format!("invalid tool name {tool:?}")));
                }
                let capability = capability_named(value)
                    .ok_or_else(|| at(format!("unknown capability {value:?}")))?;
                if tools.insert(tool.to_owned(), capability).is_some() {
                    return Err(at(format!("tool {tool} is mapped twice")));
                }
            }
            other => return Err(at(format!("unknown or repeated key {other:?}"))),
        }
    }
    if version.is_none() {
        return Err("version=1 is missing".into());
    }
    let executable = executable.ok_or("executable is missing")?;
    Ok(ServerConfig {
        name: name.to_owned(),
        executable,
        arguments,
        env,
        pass_env,
        timeout: timeout.unwrap_or(DEFAULT_UPSTREAM_TIMEOUT),
        tools,
    })
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Loads every `*.conf` under the project's `.forge/mcp/`, in name order. A missing directory
/// means no servers. Invalid declarations are returned as errors, one per file, so the valid ones
/// still load.
#[must_use]
pub fn load_server_configs(root: &Path) -> (Vec<ServerConfig>, Vec<String>) {
    let directory = root.join(MCP_SERVER_RELATIVE_PATH);
    let Ok(entries) = fs::read_dir(&directory) else {
        return (Vec::new(), Vec::new());
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("conf"))
        .collect::<Vec<_>>();
    paths.sort();
    let (mut configs, mut errors) = (Vec::new(), Vec::new());
    for path in paths {
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned();
        let result = fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())
            .and_then(|metadata| {
                if !metadata.is_file() {
                    Err("not a regular file".to_owned())
                } else if metadata.len() > MAX_SERVER_CONFIG_BYTES {
                    Err(format!("exceeds {MAX_SERVER_CONFIG_BYTES} bytes"))
                } else {
                    fs::read_to_string(&path).map_err(|error| error.to_string())
                }
            })
            .and_then(|text| parse_server_config(&name, &text));
        match result {
            Ok(config) => configs.push(config),
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
    (configs, errors)
}

#[cfg(test)]
mod tests {
    use super::{Capability, capability_named, parse_server_config, valid_server_name};

    /// An absolute path on every platform (`/bin/x` is not absolute on Windows).
    fn absolute() -> String {
        std::env::temp_dir().join("server").display().to_string()
    }

    #[test]
    fn capability_names_round_trip() {
        for capability in [
            Capability::ReadRepository,
            Capability::WriteOwnedPaths,
            Capability::RunLocalCommands,
            Capability::UseNetwork,
            Capability::ReadGitHub,
            Capability::WriteGitHub,
            Capability::ManageWorktrees,
            Capability::ReadSecrets,
            Capability::UseMcpTools,
            Capability::CreatePullRequest,
            Capability::MergeProtectedBranch,
            Capability::DeployStaging,
            Capability::DeployProduction,
        ] {
            assert_eq!(capability_named(capability.as_str()), Some(capability));
        }
        assert_eq!(capability_named("root"), None);
    }

    #[test]
    fn a_full_declaration_parses() {
        let text = format!(
            "version=1\n# comment\nexecutable={}\nargument=--stdio\nenv.MODE=ci\n\
             pass_env=API_TOKEN\ntimeout_ms=5000\ntool.search=read_repository\n\
             tool.open-issue=write_github\n",
            absolute()
        );
        let config = parse_server_config("docs-search", &text).expect("valid");
        assert_eq!(config.arguments, ["--stdio"]);
        assert_eq!(config.env["MODE"], "ci");
        assert_eq!(config.pass_env, ["API_TOKEN"]);
        assert_eq!(config.timeout.as_millis(), 5000);
        assert_eq!(config.tools["search"], Capability::ReadRepository);
        assert_eq!(config.tools["open-issue"], Capability::WriteGitHub);
    }

    #[test]
    fn bad_declarations_are_refused_with_their_line() {
        let exe = absolute();
        let base = format!("version=1\nexecutable={exe}\n");
        let cases = [
            (format!("executable={exe}\n"), "version=1 is missing"),
            ("version=1\n".to_owned(), "executable is missing"),
            (
                "version=1\nexecutable=bin/x\n".to_owned(),
                "line 2: executable must be an absolute path",
            ),
            (
                format!("version=2\nexecutable={exe}\n"),
                "line 1: unsupported version",
            ),
            (
                format!("{base}colour=blue\n"),
                "line 3: unknown or repeated key",
            ),
            (
                format!("{base}executable={exe}\n"),
                "line 3: unknown or repeated key",
            ),
            (
                format!("{base}tool.bad name=read_repository\n"),
                "line 3: invalid tool name",
            ),
            (format!("{base}tool.x=root\n"), "line 3: unknown capability"),
            (
                format!("{base}tool.x=read_repository\ntool.x=use_network\n"),
                "line 4: tool x is mapped twice",
            ),
            (
                format!("{base}timeout_ms=0\n"),
                "line 3: timeout_ms must be a positive",
            ),
            (
                format!("{base}timeout_ms=600001\n"),
                "line 3: timeout_ms exceeds",
            ),
            (
                format!("{base}env.1BAD=x\n"),
                "line 3: invalid variable name",
            ),
            (format!("{base}nonsense\n"), "line 3: expected key=value"),
        ];
        for (text, needle) in &cases {
            let error = parse_server_config("s", text).expect_err(text);
            assert!(error.contains(needle), "{text:?}: {error}");
        }
        assert!(!valid_server_name("Has_Upper"));
        assert!(!valid_server_name("a__b"));
        assert!(parse_server_config("a__b", &base).is_err());
    }
}
