//! `famp pair` subcommand surface — cross-person trust bootstrap via a
//! five-word texted code (Phase 18, PAIR-01/06).
//!
//! The reference-transport decision is recorded in `18-01-PLAN.md` Task 1
//! (Option A: a dedicated unauthenticated `POST /famp/v1/pair/redeem`
//! route on the inviter's own gateway).
//!
//! Distinct from `famp peer export`/`import` (Phase 8): that path pastes a
//! raw key blob out-of-band and TOFU-pins it on parse — silent-accept.
//! Pairing instead requires the redeemer to type a texted code
//! (`famp pair redeem`, stdin-only, PAIR-06) that proves possession of
//! whatever the human out-of-band channel actually delivered, and the
//! inviter's own side ONLY pins after an explicit `famp pair status` run
//! that first prints the redeemer's identity (asymmetric completion,
//! PAIR-07 — the pin never happens inside the redemption endpoint itself
//! or a background loop).
//!
//! `invite`/`status` are sync (pure local file I/O); `redeem` performs an
//! outbound HTTPS POST and is therefore async — dispatched via
//! `block_on_async` at the top-level `cli::run` match (`Commands::Pair`),
//! not here.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod invite;
pub mod redeem;
pub mod revoke;
pub mod status;

#[derive(Args, Debug)]
pub struct PairArgs {
    #[command(subcommand)]
    pub command: PairSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum PairSubcommand {
    /// Generate a five-word invite code for a principal (PAIR-01) and
    /// persist a `Pending` invite record. Prints ONE artifact: install
    /// instructions, with the code on the LAST line.
    Invite(invite::PairInviteArgs),
    /// Redeem a five-word code typed at a stdin prompt (PAIR-06: no
    /// positional argument, no code-bearing flag — a code-shaped
    /// invocation is rejected by clap before any code is ever read).
    /// POSTs the redemption to the inviter's gateway and, on a verified
    /// response, pins the inviter's key into this machine's own peer
    /// keyring.
    Redeem(redeem::PairRedeemArgs),
    /// Poll the invite store for `Redeemed` records and pin the
    /// redeemer's key (PAIR-01/PAIR-07) — the ONLY place the inviter-side
    /// pin ever happens. Prints the redeemer's principal and key_id
    /// BEFORE pinning.
    Status(status::PairStatusArgs),
    /// Kill an outstanding invite (by id) or every currently `Pending`
    /// invite before its 24-hour window closes (PAIR-03's explicit
    /// `famp pair revoke` clause). Durable across a gateway restart —
    /// the kill lives in the same persisted `pairing.json` record.
    Revoke(revoke::PairRevokeArgs),
}

/// Async dispatcher — `redeem` needs a tokio runtime for its outbound
/// HTTPS POST; `invite`/`status` are plain sync file I/O wrapped in an
/// `async fn` so all three variants share one dispatch site.
///
/// Not `Send`: this future's `Redeem` arm awaits `redeem::run`, whose own
/// future is deliberately `!Send` (see `redeem::run`'s doc comment for
/// why that is benign — it never crosses a thread boundary because
/// `block_on_async` never spawns it).
#[allow(clippy::future_not_send)]
pub async fn run(args: PairArgs) -> Result<(), CliError> {
    match args.command {
        PairSubcommand::Invite(args) => invite::run(&args),
        PairSubcommand::Redeem(args) => redeem::run(&args).await,
        PairSubcommand::Status(args) => status::run(&args),
        PairSubcommand::Revoke(args) => revoke::run(&args),
    }
}

/// RFC 3339 (second-precision, `Z`-suffixed) timestamp built positionally
/// so the function is infallible.
///
/// Same style as `crate::cli::peer::rotate::now_canonical_utc` (each file
/// keeps its own copy rather than adding a shared-helper indirection for
/// one timestamp formatter; this is the ONE copy shared by every `pair`
/// subcommand).
#[must_use]
pub fn now_canonical_utc() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}
