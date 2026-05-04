#![forbid(unsafe_code)]

// The `famp` binary is a thin wrapper around `famp::cli::run`. The lib crate
// pulls in the full workspace dep set; the bin target only references `clap`
// and `famp`. Silence workspace `unused_crate_dependencies` for everything
// else the lib pulls in transitively.
#[cfg(test)]
use assert_cmd as _;
use axum as _;
use base64 as _;
use dirs as _;
use ed25519_dalek as _;
use famp_bus as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
#[cfg(test)]
use famp_keyring as _;
use famp_taskdir as _;
use famp_transport as _;
#[cfg(test)]
use famp_transport_http as _;
use hex as _;
use humantime as _;
#[cfg(test)]
use insta as _;
use nix as _;
use rand as _;
use regex as _;
#[cfg(test)]
use reqwest as _;
use serde as _;
use serde_json as _;
use sha2 as _;
#[cfg(test)]
use temp_env as _;
use tempfile as _;
use thiserror as _;
use time as _;
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;
use which as _;

use clap::Parser;

fn main() {
    let cli = famp::cli::Cli::parse();
    if let Err(e) = famp::cli::run(cli) {
        eprintln!("{e}");
        let mut src = std::error::Error::source(&e);
        while let Some(s) = src {
            eprintln!("  caused by: {s}");
            src = s.source();
        }
        std::process::exit(1);
    }
}
