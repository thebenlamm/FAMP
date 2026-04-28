//! Body schema module.
//!
//! Exposes the sealed `BodySchema` trait and the six shipped body types.
//! v0.5.2 ships exactly six implementers: `RequestBody`, `CommitBody`, `DeliverBody`,
//! `AckBody`, `ControlBody`, `AuditLogBody`. The trait is
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
/// v0.5.2 ships exactly six implementers (`RequestBody`, `CommitBody`, `DeliverBody`,
/// `AckBody`, `ControlBody`, `AuditLogBody`).
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
pub mod audit_log;
pub mod bounds;
pub mod commit;
pub mod control;
pub mod deliver;
pub mod request;

pub use ack::{AckBody, AckDisposition};
pub use audit_log::AuditLogBody;
pub use bounds::{Bounds, Budget};
pub use commit::CommitBody;
pub use control::{ControlAction, ControlBody, ControlDisposition, ControlTarget};
pub use deliver::{Artifact, DeliverBody, ErrorCategory, ErrorDetail, TerminalStatus};
pub use request::RequestBody;

// Sealed impls — exactly six (v0.5.2 added audit_log per spec §8a.6).
impl private::Sealed for RequestBody {}
impl private::Sealed for CommitBody {}
impl private::Sealed for DeliverBody {}
impl private::Sealed for AckBody {}
impl private::Sealed for ControlBody {}
impl private::Sealed for AuditLogBody {}
