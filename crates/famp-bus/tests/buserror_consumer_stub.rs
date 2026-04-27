#![deny(unreachable_patterns)]
#![allow(clippy::match_same_arms, unused_crate_dependencies)]

use famp_bus::BusErrorKind;

const fn describe_buserror(k: BusErrorKind) -> &'static str {
    match k {
        BusErrorKind::NotRegistered => "not_registered",
        BusErrorKind::NameTaken => "name_taken",
        BusErrorKind::ChannelNameInvalid => "channel_name_invalid",
        BusErrorKind::NotJoined => "not_joined",
        BusErrorKind::EnvelopeInvalid => "envelope_invalid",
        BusErrorKind::EnvelopeTooLarge => "envelope_too_large",
        BusErrorKind::TaskNotFound => "task_not_found",
        BusErrorKind::BrokerProtoMismatch => "broker_proto_mismatch",
        BusErrorKind::BrokerUnreachable => "broker_unreachable",
        BusErrorKind::Internal => "internal",
    }
}

#[test]
fn all_variants_described() {
    for kind in BusErrorKind::ALL {
        assert!(!describe_buserror(kind).is_empty());
    }
}
