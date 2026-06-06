//! `InspectKind::Messages` handler and the shared [`message_row`] projection.

use famp_bus::BrokerStateView;
use famp_envelope::EnvelopeView;
use famp_inspect_proto::{InspectMessagesReply, MessageListReply, MessageRow};
use sha2::{Digest, Sha256};

use crate::parse::{derive_fsm_state, parse_rfc3339_to_epoch};
use crate::BrokerCtx;

/// INSP-MSG-01..03 dispatch. Body bytes never traverse the wire - only
/// their length and a 12-hex sha256 prefix.
pub fn inspect_messages(
    _state: &BrokerStateView,
    ctx: &BrokerCtx,
    req: &famp_inspect_proto::InspectMessagesRequest,
) -> InspectMessagesReply {
    let Some(snapshot) = ctx.message_data.as_ref() else {
        return InspectMessagesReply::List(MessageListReply { rows: vec![] });
    };

    let mut entries: Vec<&serde_json::Value> = req.to.as_deref().map_or_else(
        || snapshot.by_recipient.values().flatten().collect(),
        |name| {
            snapshot
                .by_recipient
                .get(name)
                .map(|values| values.iter().collect())
                .unwrap_or_default()
        },
    );
    entries.sort_by_key(|env| {
        env.get("ts")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_rfc3339_to_epoch)
            .unwrap_or(0)
    });

    let tail = usize::try_from(req.tail.unwrap_or(50)).unwrap_or(usize::MAX);
    let start = entries.len().saturating_sub(tail);
    let rows = entries[start..]
        .iter()
        .map(|env| message_row(env))
        .collect();

    InspectMessagesReply::List(MessageListReply { rows })
}

/// Project an envelope JSON value into a [`MessageRow`].
///
/// Uses the exact same field-extraction logic the inspector RPC uses for
/// `InspectKind::Messages`. Exposed so callers that need to derive rows
/// from raw mailbox JSONL (e.g. `famp_verify` reading mailbox files
/// directly to cover offline recipients) stay in lockstep with the
/// inspector's wire schema — no schema drift between the RPC path and
/// the direct-read path.
///
/// Adversarial review finding 2 (high): `famp_verify` previously
/// bounced through `InspectKind::Messages`, which only scans mailboxes
/// for currently-registered identities. Reading mailbox files directly
/// fixes the offline-recipient miss but requires re-using this row
/// construction so the output shape stays identical.
#[must_use]
pub fn message_row(env: &serde_json::Value) -> MessageRow {
    let view = EnvelopeView::new(env);
    let body_value = view.body().cloned().unwrap_or(serde_json::Value::Null);
    let body_bytes_vec = famp_canonical::canonicalize(&body_value).unwrap_or_default();
    let digest = Sha256::digest(&body_bytes_vec);

    MessageRow {
        sender: view.from_str().unwrap_or("").to_string(),
        recipient: view.to_str().unwrap_or("").to_string(),
        task_id: view.task_id().unwrap_or_default(),
        class: view.class().unwrap_or("").to_string(),
        state: derive_fsm_state(env),
        timestamp: env
            .get("ts")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        body_bytes: body_bytes_vec.len() as u64,
        body_sha256_prefix: hex::encode(&digest[..6]),
    }
}
