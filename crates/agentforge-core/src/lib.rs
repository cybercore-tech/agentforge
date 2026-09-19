//! Core domain primitives for AgentForge.
//!
//! Phase 0 keeps the core model provider-neutral and dependency-light.

/// Provider-neutral agent governance contracts.
pub mod agent;

/// Human-readable product name.
pub const PRODUCT_NAME: &str = "AgentForge";

/// Returns the workspace package version.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::{PRODUCT_NAME, version};

    #[test]
    fn product_identity_is_stable() {
        assert_eq!(PRODUCT_NAME, "AgentForge");
        assert!(!version().is_empty());
    }
}
