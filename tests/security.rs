//! Top-level harness for the security test suite.
//!
//! Registers every module under `tests/security/`.

#[path = "security/malformed_inputs.rs"]
mod malformed_inputs;

#[path = "security/protected_content.rs"]
mod protected_content;
