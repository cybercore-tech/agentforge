//! Per-worker secrets for the remote-worker channel (P4-M007).
//!
//! A worker's secret is 32 random bytes as 64 lowercase hex characters, stored beside its profile
//! in `.forge/workers/<worker-id>.secret`. It authenticates API requests as that worker, on top of
//! the GhostPort machine key that carries them. Secrets are compared in constant time, and on Unix
//! a secret file readable by group or others is refused, like an SSH private key.

use crate::OperatorError;
use crate::leases::{WORKER_PROFILE_RELATIVE_PATH, load_workers};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Hex characters in a worker secret (256 bits).
pub const WORKER_SECRET_HEX_LEN: usize = 64;

/// Path of a worker's secret file.
#[must_use]
pub fn worker_secret_path(root: impl AsRef<Path>, worker_id: &str) -> PathBuf {
    root.as_ref()
        .join(WORKER_PROFILE_RELATIVE_PATH)
        .join(format!("{worker_id}.secret"))
}

/// Generates and stores a new secret for a registered worker, returning it once.
///
/// Refuses an unregistered worker and an existing secret (delete the file to rotate).
pub fn enroll_worker(root: impl AsRef<Path>, worker_id: &str) -> Result<String, OperatorError> {
    let root = root.as_ref();
    if !load_workers(root)?
        .iter()
        .any(|worker| worker.worker_id().as_str() == worker_id)
    {
        return Err(OperatorError::new(format!(
            "worker is not registered: {worker_id} (add .forge/workers/{worker_id}.conf first)"
        )));
    }
    let secret = random_secret()?;
    let path = worker_secret_path(root, worker_id);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            OperatorError::new(format!(
                "worker {worker_id} is already enrolled ({}); delete it to rotate the secret",
                path.display()
            ))
        } else {
            OperatorError::new(error.to_string())
        }
    })?;
    file.write_all(format!("{secret}\n").as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| OperatorError::new(error.to_string()))?;
    Ok(secret)
}

/// Loads a worker's stored secret. `Ok(None)` means the worker is not enrolled.
pub fn load_worker_secret(
    root: impl AsRef<Path>,
    worker_id: &str,
) -> Result<Option<String>, OperatorError> {
    let path = worker_secret_path(root, worker_id);
    if !path.exists() {
        return Ok(None);
    }
    read_secret_file(&path).map(Some)
}

/// Reads and validates a secret file (either side of the channel).
pub fn read_secret_file(path: impl AsRef<Path>) -> Result<String, OperatorError> {
    let path = path.as_ref();
    let invalid =
        |reason: &str| OperatorError::new(format!("secret file {}: {reason}", path.display()));
    let metadata = fs::symlink_metadata(path).map_err(|error| invalid(&error.to_string()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid("is not a regular file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(invalid(
                "is readable by group or others; restrict it with chmod 600",
            ));
        }
    }
    if metadata.len() > 256 {
        return Err(invalid("is too large"));
    }
    let text = fs::read_to_string(path).map_err(|error| invalid(&error.to_string()))?;
    let secret = text.trim();
    if !is_valid_secret(secret) {
        return Err(invalid("must hold 64 lowercase hex characters"));
    }
    Ok(secret.to_owned())
}

/// Whether a value has the shape of a worker secret.
#[must_use]
pub fn is_valid_secret(value: &str) -> bool {
    value.len() == WORKER_SECRET_HEX_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Compares two secrets without an early exit on the first differing byte.
#[must_use]
pub fn secrets_match(expected: &str, supplied: &str) -> bool {
    let (expected, supplied) = (expected.as_bytes(), supplied.as_bytes());
    let mut difference = expected.len() ^ supplied.len();
    for (index, byte) in expected.iter().enumerate() {
        let other = supplied.get(index).copied().unwrap_or(0);
        difference |= usize::from(byte ^ other);
    }
    difference == 0
}

#[cfg(unix)]
fn random_secret() -> Result<String, OperatorError> {
    let mut bytes = [0_u8; WORKER_SECRET_HEX_LEN / 2];
    fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| OperatorError::new(format!("cannot read /dev/urandom: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(not(unix))]
fn random_secret() -> Result<String, OperatorError> {
    Err(OperatorError::new(
        "automatic enrollment needs /dev/urandom; on this platform write 32 random bytes as 64 \
         lowercase hex characters to .forge/workers/<worker-id>.secret yourself",
    ))
}
