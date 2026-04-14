//! `FAMP_HOME` resolution — Task 2 placeholder (replaced in next task).

use crate::cli::error::CliError;
use std::path::PathBuf;

#[allow(clippy::missing_const_for_fn)]
pub fn resolve_famp_home() -> Result<PathBuf, CliError> {
    Err(CliError::HomeNotSet)
}
