//! `famp_register` MCP tool — binds the current MCP session to a FAMP
//! identity by name.
//!
//! ## Resolution
//!
//! `identity` (a string) is validated against `[A-Za-z0-9_-]+`, then
//! resolved to `$FAMP_LOCAL_ROOT/agents/<identity>/`. The directory
//! must exist and contain a readable `config.toml`. On success, the
//! session binding is set via the `session` module's `set` accessor.
//!
//! ## Idempotency
//!
//! Calling `famp_register` with the same identity twice is a no-op
//! success. Calling with a different identity replaces the binding
//! deterministically (per CONTEXT.md "`famp_register` always wins").

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::mcp::session::{self, BindingSource, IdentityBinding};

/// Validate that `name` matches the regex `[A-Za-z0-9_-]+` (one or more
/// chars from the allowed set, no other chars).
fn validate_identity_name(name: &str) -> Result<(), CliError> {
    if name.is_empty() {
        return Err(CliError::InvalidIdentityName {
            name: name.to_string(),
            reason: "identity name is empty".to_string(),
        });
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(CliError::InvalidIdentityName {
                name: name.to_string(),
                reason: "identity name must match [A-Za-z0-9_-]+".to_string(),
            });
        }
    }
    Ok(())
}

/// Resolve the identity directory under `local_root/agents/<name>/`
/// and verify `config.toml` exists. Returns the resolved home path.
async fn resolve_identity_dir(local_root: &Path, name: &str) -> Result<PathBuf, CliError> {
    let home = local_root.join("agents").join(name);
    let config_path = home.join("config.toml");
    match tokio::fs::metadata(&config_path).await {
        Ok(md) if md.is_file() => Ok(home),
        Ok(_) => Err(CliError::UnknownIdentity {
            name: name.to_string(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(CliError::UnknownIdentity {
            name: name.to_string(),
        }),
        Err(source) => Err(CliError::Io {
            path: config_path,
            source,
        }),
    }
}

/// Dispatch a `famp_register` tool call.
pub async fn call(local_root: &Path, input: &Value) -> Result<Value, CliError> {
    let identity = input["identity"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_register: missing required field 'identity'".to_string(),
        })?
        .to_string();

    validate_identity_name(&identity)?;
    let home = resolve_identity_dir(local_root, &identity).await?;

    // Idempotency: if current binding already matches the requested
    // identity AND home, return success without touching the lock's
    // value (no replace, no allocation churn).
    if let Some(current) = session::current().await {
        if current.identity == identity && current.home == home {
            return Ok(serde_json::json!({
                "identity": identity,
                "home":     home.to_string_lossy(),
                "source":   "explicit",
            }));
        }
    }

    let binding = IdentityBinding {
        identity: identity.clone(),
        home: home.clone(),
        source: BindingSource::Explicit,
    };
    let _prev = session::set(binding).await;

    Ok(serde_json::json!({
        "identity": identity,
        "home":     home.to_string_lossy(),
        "source":   "explicit",
    }))
}
