//! Body schema module.
//!
//! Exposes the sealed `BodySchema` trait and the five shipped body types.
//! v0.7 ships exactly five implementers: `RequestBody`, `CommitBody`, `DeliverBody`,
//! `AckBody`, `ControlBody`. Adding a sixth is a v0.8+ breaking change. The trait is
//! sealed via a private supertrait so downstream crates cannot declare new body types.
//!
//! Each implementer carries its own `CLASS: MessageClass` and `SCOPE: EnvelopeScope`
//! associated constants per CONTEXT.md D-B1 / D-C1.

use crate::{EnvelopeScope, MessageClass};
use serde::{de::DeserializeOwned, Serialize};

mod private {
    pub trait Sealed {}
}

/// Sealed trait implemented by every shipped body variant.
///
/// v0.7 ships exactly five implementers (`RequestBody`, `CommitBody`, `DeliverBody`,
/// `AckBody`, `ControlBody`). Adding a sixth is a v0.8+ breaking change.
///
/// ```compile_fail
/// use famp_envelope::body::BodySchema;
/// use serde::{Serialize, Deserialize};
/// #[derive(Serialize, Deserialize)]
/// struct FakeBody;
/// impl BodySchema for FakeBody {
///     const CLASS: famp_envelope::MessageClass = famp_envelope::MessageClass::Request;
///     const SCOPE: famp_envelope::EnvelopeScope = famp_envelope::EnvelopeScope::Standalone;
/// }
/// ```
pub trait BodySchema:
    Serialize + DeserializeOwned + Clone + private::Sealed + Sized + 'static
{
    const CLASS: MessageClass;
    const SCOPE: EnvelopeScope;

    /// Post-deserialization cross-field validation hook.
    ///
    /// Called by `SignedEnvelope::decode_value` after the typed deserialize
    /// plus class/scope cross-check. Default = no-op. Override for bodies
    /// that need to inspect envelope-level fields such as `terminal_status`,
    /// or run internal rules such as `Bounds::validate`.
    #[allow(unused_variables)]
    fn post_decode_validate(
        &self,
        envelope_terminal_status: Option<&deliver::TerminalStatus>,
    ) -> Result<(), crate::EnvelopeDecodeError> {
        Ok(())
    }
}

pub mod ack;
pub mod bounds;
pub mod commit;
pub mod control;
pub mod deliver;
pub mod request;

pub use ack::{AckBody, AckDisposition};
pub use bounds::{Bounds, Budget};
pub use commit::CommitBody;
pub use control::{ControlAction, ControlBody, ControlDisposition, ControlTarget};
pub use deliver::{Artifact, DeliverBody, ErrorCategory, ErrorDetail, TerminalStatus};
pub use request::RequestBody;

// Sealed impls — exactly five, locked. Adding a sixth requires v0.8+ work.
impl private::Sealed for RequestBody {}
impl private::Sealed for CommitBody {}
impl private::Sealed for DeliverBody {}
impl private::Sealed for AckBody {}
impl private::Sealed for ControlBody {}
