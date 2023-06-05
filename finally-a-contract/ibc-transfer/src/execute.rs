use std::ops::Mul;

use cosmwasm_std::{
    coins, Addr, BankMsg, Coin, DepsMut, Env, MessageInfo, Reply, Response, SubMsg, SubMsgResponse,
    SubMsgResult, Deps, to_binary,
};
use schemars::_serde_json::json;
use should_load::assignment::MapShouldLoad;

use crate::consts::MsgReplyID;
use crate::ibc_lifecycle::create_recovery;
use crate::proto::MsgTransferResponse;
use crate::state::ibc::IBCTransfer;
use crate::state::{
    ibc, TransferMsgReplyState, CHANNELS, INFLIGHT_PACKETS, PORTS, RECOVERY_STATES,
    SEND_EXTERNAL_TOKENS_REPLY_STATE, TRANSFER_REPLY_STATE, CONFIG,
};
use crate::{proto, state::EXTERNAL_ADDRESSES, ContractError};

pub fn transfer_ibc_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Coin,
    port: String,
    channel: String,
    recipient: String,
) -> Result<Response, ContractError> {
    // make sure some token is sent to this contract
    let sent_funds = info
        .funds
        .iter()
        .find(|x| x.denom == amount.denom.clone())
        .ok_or_else(|| ContractError::NoFunds {
            denom: amount.denom.clone(),
        })?;

    // check if the sender sent enough to cover the transfer
    if sent_funds.amount < amount.amount {
        return Err(ContractError::NotEnoughFunds {
            sent: sent_funds.amount.into(),
            required: amount.amount.into(),
        });
    }

    // get the recipient from state
    let recipient = EXTERNAL_ADDRESSES
        .should_load(deps.storage, recipient)
        .unwrap_or_else(|_| {
            EXTERNAL_ADDRESSES
                .should_load(deps.storage, "default".to_string())
                .unwrap()
        });
    let port = PORTS.should_load(deps.storage, port).unwrap_or_else(|_| {
        PORTS
            .should_load(deps.storage, "default".to_string())
            .unwrap()
    });
    let channel = CHANNELS
        .should_load(deps.storage, channel)
        .unwrap_or_else(|_| {
            CHANNELS
                .should_load(deps.storage, "default".to_string())
                .unwrap()
        });

    let memo_msg = serde_json_wasm::to_string(&json!({
        "ibc_callback": env.contract.address.to_string()
    }))?;
    // build the transfer message
    let transfer_msg = proto::MsgTransfer {
        source_port: port,
        source_channel: channel.to_string(),
        token: Some(amount.clone().into()),
        sender: env.contract.address.to_string(),
        receiver: recipient,
        timeout_height: None,
        timeout_timestamp: None,
        memo: memo_msg,
    };

    // Check that there isn't anything stored in TRANSFER_REPLY_STATES. If there
    // is, it means that the contract is already waiting for a reply and should
    // not override the stored state. This should not happen since the assumption
    // state no race conditions.
    if TRANSFER_REPLY_STATE.may_load(deps.storage)?.is_some() {
        return Err(ContractError::ContractLocked {
            msg: "Already waiting for a reply".to_string(),
        });
    }

    // Store the ibc send information
    // so that it can be handled by the response
    TRANSFER_REPLY_STATE.save(
        deps.storage,
        &TransferMsgReplyState {
            channel_id: transfer_msg.source_channel.clone(),
            to_address: transfer_msg.receiver.clone(),
            amount: amount.clone().amount.into(),
            denom: amount.clone().denom,
            sender: info.sender,
        },
    )?;

    return Ok(Response::new()
        .set_data(to_binary(&transfer_msg).unwrap())
        .add_attribute("ibc_message", format!("{:?}", transfer_msg))
        .add_submessage(SubMsg::reply_on_success(
            transfer_msg,
            MsgReplyID::TransferIbc.repr(),
        )));
}

// Included here so it's closer to the trait that needs it.
use ::prost::Message; // Proveides ::decode() for MsgTransferResponse

// The ibc transfer has been "sent" successfully. We create an inflight packet
// in storage for potential recovery.
// If recovery is set to "do_nothing", we just return a response.
pub fn handle_transfer_ibc_token_reply(
    deps: DepsMut,
    msg: cosmwasm_std::Reply,
) -> Result<Response, ContractError> {
    // Parse the result from the underlying chain call (IBC send)
    let SubMsgResult::Ok(SubMsgResponse { data: Some(b), .. }) = msg.result else {
        return Err(ContractError::FailedIBCTransfer { msg: format!("failed reply: {:?}", msg.result) })
    };

    // The response contains the packet sequence. This is needed to be able to
    // ensure that, if there is a delivery failure, the packet that failed is
    // the same one that we stored recovery information for
    let response =
        MsgTransferResponse::decode(&b[..]).map_err(|_e| ContractError::FailedIBCTransfer {
            msg: format!("could not decode response: {b}"),
        })?;

    // Get the stored context state
    let TransferMsgReplyState {
        channel_id,
        to_address,
        amount,
        denom,
        sender: recovery_addr,
    } = TRANSFER_REPLY_STATE.load(deps.storage)?;
    TRANSFER_REPLY_STATE.remove(deps.storage);

    // Store sent IBC transfer so that it
    // can later be recovered by the sender
    let recovery = ibc::IBCTransfer {
        recovery_addr,
        channel_id: channel_id.clone(),
        sequence: response.sequence,
        amount,
        denom: denom.clone(),
        status: ibc::PacketLifecycleStatus::Sent,
    };

    // Save as in-flight to be able to manipulate when the ack/timeout is received
    INFLIGHT_PACKETS.save(deps.storage, (&channel_id, response.sequence), &recovery)?;

    Ok(Response::new()
        .add_attribute("status", "ibc_message_created")
        .add_attribute("amount", amount.to_string())
        .add_attribute("denom", denom)
        .add_attribute("channel", channel_id)
        .add_attribute("receiver", to_address))
}

pub fn handle_send_external_tokens_reply(
    deps: DepsMut,
    msg: Reply,
) -> Result<Response, ContractError> {
    // Parse the result from bank sub-messages
    if let SubMsgResult::Ok(SubMsgResponse { data: Some(_), .. }) = msg.result {
        // bank transfer was successful so we remove the inflight packet
        let eti_token = SEND_EXTERNAL_TOKENS_REPLY_STATE
            .load(deps.storage)
            .map_err(|_| ContractError::NoExternalTokensInflightPacket)?;
        INFLIGHT_PACKETS.remove(deps.storage, (&eti_token.channel_id, eti_token.sequence));
        Ok(Response::new()
            .add_attribute("msg", "value sent")
            .add_attribute("recepient", eti_token.recovery_addr)
            .add_attribute("amount", eti_token.amount.to_string()))
    } else {
        // Get the stored context state
        let eti_token = SEND_EXTERNAL_TOKENS_REPLY_STATE
            .load(deps.storage)
            .map_err(|_| ContractError::NoExternalTokensInflightPacket)?;

        // remove the inflight packet
        INFLIGHT_PACKETS.remove(deps.storage, (&eti_token.channel_id, eti_token.sequence));

        // create a recovery for the original sender of the packet.
        let recovery_addr = create_recovery(
            deps,
            eti_token,
            ibc::PacketLifecycleStatus::SendingExternalTokensFailure,
        )?;
        Ok(Response::new()
            .add_attribute("msg", "recovery stored")
            .add_attribute("recovery_addr", recovery_addr))
    }
}

// Handle receiving token from external addresses and sending to the appropriate recipient
pub fn receive_ibc_token(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    channel: String,
    sequence: u64,
) -> Result<Response, ContractError> {
    // Get the stored context state
    let recovery = INFLIGHT_PACKETS
        .load(deps.storage, (&channel, sequence))
        .map_err(|_| ContractError::NoInflightPacket {
            channel_id: channel.clone(),
            sequence,
        })?;

    // Check that the packet is in the correct state
    if recovery.status == ibc::PacketLifecycleStatus::AwaitingResponse {
        // Get the sent funds from the info
        let sent_funds = info
            .funds
            .iter()
            .find(|c| c.denom == recovery.denom)
            .ok_or_else(|| ContractError::NoExternalFunds {
                denom: recovery.denom.clone(),
            })?;
        // make sure funds can cover transfer
        if sent_funds.amount.u128().ge(&recovery.amount.mul(2)) {
            // Send the funds to the recipient
            send_external_tokens(deps, recovery)
        } else {
            return Err(ContractError::InsufficientExternalFunds {
                sent: sent_funds.amount.u128(),
                required: recovery.amount.mul(2),
            });
        }
    } else {
        return Err(ContractError::InvalidInflightPacketState {
            channel_id: channel.clone(),
            sequence,
            status: recovery.status.to_string(),
        });
    }
}

/// Transfers any received INFLIGHT_PACKETS tokens to sender.
pub fn send_external_tokens(deps: DepsMut, mut packet: IBCTransfer) -> Result<Response, ContractError> {
    // TODO: check that we don't have a context state already set
    packet.status = ibc::PacketLifecycleStatus::SendingExternalTokens;
    packet.amount = packet.amount.mul(2);
    SEND_EXTERNAL_TOKENS_REPLY_STATE.save(deps.storage, &packet)?;

    let msg = BankMsg::Send {
        to_address: packet.recovery_addr.to_string(),
        amount: coins(packet.amount, packet.denom),
    };
    // create reply context
    Ok(Response::new().add_submessage(SubMsg::reply_always(msg, MsgReplyID::SendAddr.repr())))
}

/// Transfers any tokens stored in RECOVERY_STATES [sender] to the sender.
pub fn recover(deps: DepsMut, sender: Addr) -> Result<Response, ContractError> {
    let recoveries = RECOVERY_STATES.load(deps.storage, &sender)?;
    // Remove the recoveries from the store. If the sends fail, the whole tx should be reverted.
    RECOVERY_STATES.remove(deps.storage, &sender);
    let msgs = recoveries.into_iter().map(|r| BankMsg::Send {
        to_address: r.recovery_addr.into(),
        amount: coins(r.amount, r.denom),
    });
    Ok(Response::new().add_messages(msgs))
}

/// Add in external address to send tokens to
pub fn add_external_address (deps: DepsMut, info: MessageInfo, alias: String, addr: String) -> Result<Response, ContractError>{
    // add new external address into state
    validate_owner(deps.as_ref(), info.sender)?;
    EXTERNAL_ADDRESSES.save(deps.storage, alias, &addr).map_err(|err| ContractError::Std(err)).map(|_| Response::default())
}

pub fn validate_owner(deps: Deps, addr: Addr) -> Result<(), ContractError> {
    let owner = CONFIG.load(deps.storage).map_err(|err| ContractError::Std(err))?;

    if owner.owner.ne(&addr) {
        Err(ContractError::Unauthorized {  })
    } else {
        Ok(())
    }
    
}

/// Add in port to send tokens to
pub fn add_port (deps: DepsMut, info: MessageInfo, alias: String, addr: String) -> Result<Response, ContractError>{
    // add new external address into state
    validate_owner(deps.as_ref(), info.sender)?;
    PORTS.save(deps.storage, alias, &addr).map_err(|err| ContractError::Std(err)).map(|_| Response::default())
}

/// Add in channel to send tokens over
pub fn add_channel (deps: DepsMut, info: MessageInfo, alias: String, channel_id: u32) -> Result<Response, ContractError>{
    // add new external address into state
    validate_owner(deps.as_ref(), info.sender)?;
    CHANNELS.save(deps.storage, alias, &channel_id).map_err(|err| ContractError::Std(err)).map(|_| Response::default())
}
