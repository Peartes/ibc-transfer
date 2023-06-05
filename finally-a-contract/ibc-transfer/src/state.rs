use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::{Map, Item};

use self::ibc::IBCTransfer;

#[cw_serde]
pub struct Config {
    pub owner: Addr,
}

#[cw_serde]
pub struct TransferMsgReplyState {
    pub channel_id: String,
    pub to_address: String,
    pub amount: u128,
    pub denom: String,
    pub sender: Addr,
}

pub mod ibc {
    use std::fmt;

    use super::*;

    #[cw_serde]
    pub enum PacketLifecycleStatus {
        Sent,
        AckSuccess,
        AckFailure,
        TimedOut,
        AwaitingResponse,
        SendingExternalTokens,
        SendingExternalTokensFailure,
    }

    impl fmt::Display for PacketLifecycleStatus {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                PacketLifecycleStatus::Sent => write!(f, "Sent"),
                PacketLifecycleStatus::AckSuccess => write!(f, "AckSuccess"),
                PacketLifecycleStatus::AckFailure => write!(f, "AckFailure"),
                PacketLifecycleStatus::TimedOut => write!(f, "TimedOut"),
                PacketLifecycleStatus::AwaitingResponse => write!(f, "AwaitingResponse"),
                PacketLifecycleStatus::SendingExternalTokens => write!(f, "SendingExternalTokens"),
                PacketLifecycleStatus::SendingExternalTokensFailure => write!(f, "SendingExternalTokensFailure"),
            }
        }
    }

    /// A transfer packet sent by this contract that is expected to be received but
    /// needs to be tracked in case the receive fails or times-out
    #[cw_serde]
    pub struct IBCTransfer {
        pub recovery_addr: Addr,
        pub channel_id: String,
        pub sequence: u64,
        pub amount: u128,
        pub denom: String,
        pub status: PacketLifecycleStatus,
    }
}

pub const CONFIG: Item<Config> = Item::new("config");

pub const EXTERNAL_ADDRESSES: Map<String, String> = Map::new("recipient_address");
pub const CHANNELS: Map<String, u32> = Map::new("channels");
pub const PORTS: Map<String, String> = Map::new("ports");

// save context for ibc transfer reply
pub const TRANSFER_REPLY_STATE: Item<TransferMsgReplyState> = Item::new("transfer_reply_state");
// save context for transferring external tokens to sender reply
pub const SEND_EXTERNAL_TOKENS_REPLY_STATE: Item<IBCTransfer> = Item::new("send_external_tokens_reply_state");

/// In-Flight packets by (source_channel_id, sequence)
pub const INFLIGHT_PACKETS: Map<(&str, u64), ibc::IBCTransfer> = Map::new("inflight");

/// Recovery. This tracks any recovery that an addr can execute.
pub const RECOVERY_STATES: Map<&Addr, Vec<ibc::IBCTransfer>> = Map::new("recovery");
