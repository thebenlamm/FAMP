//! `FAMP_HOME` resolution. D-07: `$FAMP_HOME` verbatim → else `$HOME/.famp`.
//! D-08: absolute paths only; no canonicalize; no tilde expansion.
//!
//! Per CD-05 / RESEARCH Pitfall 1, this is the ONLY place that reads
//! process env for the home path. Every other call site takes `&Path`
//! explicitly to avoid the `std::env::set_var` parallel-test race.

use crate::cli::error::CliError;
use std::path::PathBuf;

pub fn resolve_famp_home() -> Result<PathBuf, CliError> {
    let path: PathBuf = if let Some(v) = std::env::var_os("FAMP_HOME") {
        PathBuf::from(v)
    } else {
        let home = std::env::var_os("HOME").ok_or(CliError::HomeNotSet)?;
        PathBuf::from(home).join(".famp")
    };
    if !path.is_absolute() {
        return Err(CliError::HomeNotAbsolute { path });
    }
    Ok(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// All env cases run in one `#[test] fn` to guarantee serial execution
    /// (nextest parallelizes at the test-function level, and `std::env` is
    /// process-global). RESEARCH §Pitfall 1.
    #[test]
    fn resolves_and_rejects() {
        // Case A: FAMP_HOME set absolute
        std::env::set_var("FAMP_HOME", "/tmp/x");
        match resolve_famp_home() {
            Ok(p) => assert_eq!(p, PathBuf::from("/tmp/x")),
            other => panic!("Case A: expected Ok(/tmp/x), got {other:?}"),
        }

        // Case B: FAMP_HOME set but relative
        std::env::set_var("FAMP_HOME", "./relative");
        match resolve_famp_home() {
            Err(CliError::HomeNotAbsolute { path }) => {
                assert_eq!(path, PathBuf::from("./relative"));
            }
            other => panic!("Case B: expected HomeNotAbsolute, got {other:?}"),
        }

        // Case C: FAMP_HOME unset, HOME set
        std::env::remove_var("FAMP_HOME");
        std::env::set_var("HOME", "/home/user");
        match resolve_famp_home() {
            Ok(p) => assert_eq!(p, PathBuf::from("/home/user/.famp")),
            other => panic!("Case C: expected Ok(/home/user/.famp), got {other:?}"),
        }

        // Case D: both unset
        std::env::remove_var("FAMP_HOME");
        std::env::remove_var("HOME");
        match resolve_famp_home() {
            Err(CliError::HomeNotSet) => {}
            other => panic!("Case D: expected HomeNotSet, got {other:?}"),
        }
    }
}
