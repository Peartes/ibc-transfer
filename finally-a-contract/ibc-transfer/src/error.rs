use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.

    #[error("No funds sent from user")]
    NoFunds { denom: String },

    #[error("No funds sent from external address")]
    NoExternalFunds { denom: String },

    #[error("Not enough funds sent by external address")]
    InsufficientExternalFunds { sent: u128, required: u128 },

    #[error("Not enough funds sent by user")]
    NotEnoughFunds { sent: u128, required: u128 },

    #[error("{0}")]
    JsonSerialization(#[from] serde_json_wasm::ser::Error),

    #[error("Invalid reply ID")]
    InvalidReplyID { id: u64},

    #[error("Failed IBC transfer")]
    FailedIBCTransfer { msg: String },

    #[error("Failed external tokens transfer")]
    FailedExternalTokensTransfer { msg: String },

    #[error("Contract locked")]
    ContractLocked { msg: String },

    #[error("No inflight packet")]
    NoInflightPacket { channel_id: String, sequence: u64 },

    #[error("No external tokens inflight packet")]
    NoExternalTokensInflightPacket,

    #[error("Invalid flight packet state")]
    InvalidInflightPacketState { channel_id: String, sequence: u64, status: String},
}