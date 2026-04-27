#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies,
    clippy::match_same_arms
)]

use famp_bus::{
    encode_frame, try_decode_frame, BusMessage, BusReply, FrameError, LEN_PREFIX_BYTES,
    MAX_FRAME_BYTES,
};
use proptest::prelude::*;
use serde_json::json;

fn arb_busmessage() -> impl Strategy<Value = BusMessage> {
    prop_oneof![
        Just(BusMessage::Hello {
            bus_proto: 1,
            client: "codec-test".into(),
        }),
        Just(BusMessage::Register {
            name: "alice".into(),
            pid: 1234,
        }),
        Just(BusMessage::Send {
            to: famp_bus::Target::Agent { name: "bob".into() },
            envelope: json!({"body": "hello"}),
        }),
        Just(BusMessage::Inbox {
            since: Some(42),
            include_terminal: Some(true),
        }),
        Just(BusMessage::Await {
            timeout_ms: 250,
            task: Some(uuid::Uuid::nil()),
        }),
        Just(BusMessage::Join {
            channel: "#team".into(),
        }),
        Just(BusMessage::Leave {
            channel: "#team".into(),
        }),
        Just(BusMessage::Sessions {}),
        Just(BusMessage::Whoami {}),
    ]
}

#[test]
fn frame_prefix_len_and_decodes_sessions() {
    let framed = encode_frame(&BusMessage::Sessions {}).unwrap();
    let len = u32::from_be_bytes(framed[..LEN_PREFIX_BYTES].try_into().unwrap()) as usize;

    assert_eq!(framed.len(), LEN_PREFIX_BYTES + len);

    let (decoded, consumed) = try_decode_frame::<BusMessage>(&framed).unwrap().unwrap();
    assert_eq!(decoded, BusMessage::Sessions {});
    assert_eq!(consumed, framed.len());
}

#[test]
fn rejects_zero_length_frame() {
    let err = try_decode_frame::<BusMessage>(&[0_u8; LEN_PREFIX_BYTES]).unwrap_err();
    assert!(matches!(err, FrameError::EmptyFrame));
}

#[test]
fn rejects_too_large_frame_before_payload_allocation() {
    let oversized = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
    let err = try_decode_frame::<BusMessage>(&oversized).unwrap_err();
    assert!(matches!(err, FrameError::FrameTooLarge(n) if n == (MAX_FRAME_BYTES + 1) as u32));
}

#[test]
fn busreply_roundtrips_through_codec() {
    let reply = BusReply::AwaitTimeout {};
    let framed = encode_frame(&reply).unwrap();
    let (decoded, consumed) = try_decode_frame::<BusReply>(&framed).unwrap().unwrap();

    assert_eq!(decoded, reply);
    assert_eq!(consumed, framed.len());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn busmessage_frame_roundtrips(v in arb_busmessage()) {
        let framed = encode_frame(&v).unwrap();
        let (decoded, consumed) = try_decode_frame::<BusMessage>(&framed).unwrap().unwrap();

        prop_assert_eq!(decoded, v);
        prop_assert_eq!(consumed, framed.len());
    }

    #[test]
    fn split_reads_return_none_until_complete(v in arb_busmessage()) {
        let framed = encode_frame(&v).unwrap();

        for i in 0..framed.len() {
            prop_assert!(matches!(
                try_decode_frame::<BusMessage>(&framed[..i]),
                Ok(None)
            ));
        }

        let (decoded, consumed) = try_decode_frame::<BusMessage>(&framed).unwrap().unwrap();
        prop_assert_eq!(decoded, v);
        prop_assert_eq!(consumed, framed.len());
    }

    #[test]
    fn partial_length_prefix_returns_none(v in arb_busmessage()) {
        let framed = encode_frame(&v).unwrap();

        for k in 0..LEN_PREFIX_BYTES {
            prop_assert!(matches!(
                try_decode_frame::<BusMessage>(&framed[..k]),
                Ok(None)
            ));
        }
    }
}
