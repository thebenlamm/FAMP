//! Compile-time proof that the umbrella re-exports resolve.
//! If this file fails to compile, PR #4 Task 3 regressed.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_arguments,
    unused_crate_dependencies
)]

use famp::{canonicalize, from_slice_strict, from_str_strict};
use famp::{sign_canonical_bytes, sign_value, verify_canonical_bytes, verify_value};
use famp::{
    AnySignedEnvelope, ArtifactId, AuthorityScope, CanonicalError, CryptoError,
    EnvelopeDecodeError, EnvelopeScope, FampSignature, FampSigningKey, Instance, MessageClass,
    MessageId, Principal, ProtocolError, ProtocolErrorKind, SignedEnvelope, TerminalStatus,
    Timestamp, TrustedVerifyingKey, UnsignedEnvelope, DOMAIN_PREFIX, FAMP_SPEC_VERSION,
};

// Touch every re-exported name so `unused_imports` does not mask regressions.
#[allow(dead_code)]
fn touch_types(
    _: Option<AnySignedEnvelope>,
    _: Option<ArtifactId>,
    _: Option<AuthorityScope>,
    _: Option<CanonicalError>,
    _: Option<CryptoError>,
    _: Option<EnvelopeDecodeError>,
    _: Option<EnvelopeScope>,
    _: Option<FampSignature>,
    _: Option<FampSigningKey>,
    _: Option<Instance>,
    _: Option<MessageClass>,
    _: Option<MessageId>,
    _: Option<ProtocolError>,
    _: Option<ProtocolErrorKind>,
    _: Option<SignedEnvelope<famp_envelope::body::RequestBody>>,
    _: Option<TerminalStatus>,
    _: Option<Timestamp>,
    _: Option<TrustedVerifyingKey>,
    _: Option<UnsignedEnvelope<famp_envelope::body::RequestBody>>,
) {
    // Touch each free function by coercing to a fn pointer.
    let _: fn(&serde_json::Value) -> Result<Vec<u8>, famp_canonical::CanonicalError> = canonicalize;
    let _ = from_slice_strict::<serde_json::Value>;
    let _ = from_str_strict::<serde_json::Value>;
    let _ = sign_canonical_bytes;
    let _ = sign_value::<serde_json::Value>;
    let _ = verify_canonical_bytes;
    let _ = verify_value::<serde_json::Value>;
}

#[test]
fn reexports_compile_and_construct() {
    // Construct at least one re-exported type to prove it's real.
    let p: Principal = "agent:local/test".parse().unwrap();
    assert_eq!(p.authority(), "local");
    assert_eq!(p.name(), "test");

    // Touch the version constant so it's not dead code.
    assert!(!FAMP_SPEC_VERSION.is_empty());

    // Touch the domain prefix (12 bytes: b"FAMP-sig-v1\0").
    assert_eq!(DOMAIN_PREFIX.len(), 12);
}
