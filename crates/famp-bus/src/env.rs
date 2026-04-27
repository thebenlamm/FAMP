//! Trait bundle parameterizing the broker over its read environment.
//! D-06: broker is generic over `BrokerEnv: MailboxRead + LivenessProbe`.

use crate::{LivenessProbe, MailboxRead};

pub trait BrokerEnv: MailboxRead + LivenessProbe {}
impl<T: MailboxRead + LivenessProbe> BrokerEnv for T {}
