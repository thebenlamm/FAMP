//! Tests for `UUIDv7` ID newtypes (Phase 3 D-10..D-13).
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_core::{CommitmentId, ConversationId, MessageId, TaskId};
use std::any::TypeId;
use std::str::FromStr;

#[test]
fn new_v7_distinct_values() {
    let a = MessageId::new_v7();
    let b = MessageId::new_v7();
    assert_ne!(a, b);
}

#[test]
fn display_fromstr_roundtrip_message_id() {
    let id = MessageId::new_v7();
    let s = id.to_string();
    assert_eq!(s.len(), 36);
    let back: MessageId = s.parse().unwrap();
    assert_eq!(back, id);
}

#[test]
fn serde_roundtrip_all_four() {
    let m = MessageId::new_v7();
    let c = ConversationId::new_v7();
    let t = TaskId::new_v7();
    let k = CommitmentId::new_v7();

    let ms = serde_json::to_string(&m).unwrap();
    let cs = serde_json::to_string(&c).unwrap();
    let ts = serde_json::to_string(&t).unwrap();
    let ks = serde_json::to_string(&k).unwrap();
    // 36 uuid chars + 2 quotes == 38
    assert_eq!(ms.len(), 38);
    assert_eq!(cs.len(), 38);
    assert_eq!(ts.len(), 38);
    assert_eq!(ks.len(), 38);

    let m2: MessageId = serde_json::from_str(&ms).unwrap();
    let c2: ConversationId = serde_json::from_str(&cs).unwrap();
    let t2: TaskId = serde_json::from_str(&ts).unwrap();
    let k2: CommitmentId = serde_json::from_str(&ks).unwrap();
    assert_eq!(m2, m);
    assert_eq!(c2, c);
    assert_eq!(t2, t);
    assert_eq!(k2, k);
}

#[test]
fn rejects_unhyphenated_form() {
    // 32 chars (uuid `simple` form) must be rejected per D-11.
    let bad = r#""01890a3b1c2d7e3f8a1b0c2d3e4f5a6b""#;
    assert!(serde_json::from_str::<MessageId>(bad).is_err());
    assert!(MessageId::from_str("01890a3b1c2d7e3f8a1b0c2d3e4f5a6b").is_err());
}

#[test]
fn rejects_non_string_wire_forms() {
    // integer
    assert!(serde_json::from_str::<MessageId>("42").is_err());
    // array
    assert!(serde_json::from_str::<MessageId>("[1,2,3]").is_err());
    // object
    assert!(serde_json::from_str::<MessageId>("{}").is_err());
}

#[test]
fn accepts_canonical_hyphenated_form() {
    let canonical = r#""01890a3b-1c2d-7e3f-8a1b-0c2d3e4f5a6b""#;
    let m: MessageId = serde_json::from_str(canonical).unwrap();
    assert_eq!(format!("\"{m}\""), canonical);
}

#[test]
fn distinct_type_ids() {
    // Runtime proof that the four newtypes are distinct types.
    // The compiler would ALSO reject `let m: MessageId = TaskId::new_v7();`
    // because no `From`/`Into` conversion exists between them — that's the
    // point of D-10, and it's already enforced at compile time elsewhere.
    assert_ne!(TypeId::of::<MessageId>(), TypeId::of::<TaskId>());
    assert_ne!(TypeId::of::<MessageId>(), TypeId::of::<ConversationId>());
    assert_ne!(TypeId::of::<MessageId>(), TypeId::of::<CommitmentId>());
    assert_ne!(TypeId::of::<TaskId>(), TypeId::of::<ConversationId>());
    assert_ne!(TypeId::of::<TaskId>(), TypeId::of::<CommitmentId>());
    assert_ne!(TypeId::of::<ConversationId>(), TypeId::of::<CommitmentId>());
}
