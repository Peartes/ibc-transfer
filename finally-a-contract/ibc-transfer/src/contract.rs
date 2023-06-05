#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdResult};

use cw2::set_contract_version;

use crate::consts::MsgReplyID;
use crate::error::ContractError;
use crate::execute::{handle_send_external_tokens_reply, add_port, add_channel};
use crate::msg::{ExecuteMsg, IBCLifecycleComplete, InstantiateMsg, QueryMsg, SudoMsg};
use crate::state::{Config, CONFIG, EXTERNAL_ADDRESSES, PORTS, CHANNELS};
use crate::{execute, ibc_lifecycle};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:ibc-transfer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = Config { owner: info.sender };
    CONFIG.save(deps.storage, &state)?;
    // create defaults address, port and channel
    EXTERNAL_ADDRESSES.save(deps.storage, "default".to_string(), &"external_address".to_string())?;
    PORTS.save(deps.storage, "default".to_string(), &"transfer".to_string())?;
    CHANNELS.save(deps.storage, "default".to_string(), &0)?;

    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SendToken {
            amount,
            port,
            channel,
            recipient,
        } => execute::transfer_ibc_token(deps, env, info, amount, port, channel, recipient),
        ExecuteMsg::ReceiveToken { channel, sequence } => {
            execute::receive_ibc_token(deps, env, info, channel, sequence)
        }
        ExecuteMsg::RecoverToken {} => execute::recover(deps, info.sender),
        ExecuteMsg::AddExternalAddress { alias, address } => execute::add_external_address(deps, info, alias, address),
        ExecuteMsg::AddPort { alias, port } => add_port(deps, info, alias, port),
        ExecuteMsg::AddChannel { alias, channel_id } => add_channel(deps, info, alias, channel_id),
    }
}

pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    deps.api
        .debug(&format!("executing ibc transfer reply: {reply:?}"));
    match MsgReplyID::from_repr(reply.id) {
        Some(MsgReplyID::TransferIbc) => execute::handle_transfer_ibc_token_reply(deps, reply),
        Some(MsgReplyID::SendAddr) => handle_send_external_tokens_reply(deps, reply),
        None => Err(ContractError::InvalidReplyID { id: reply.id }),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::IBCLifecycleComplete(IBCLifecycleComplete::IBCAck {
            channel,
            sequence,
            ack,
            success,
        }) => ibc_lifecycle::receive_ack(deps, channel, sequence, ack, success),
        SudoMsg::IBCLifecycleComplete(IBCLifecycleComplete::IBCTimeout { channel, sequence }) => {
            ibc_lifecycle::receive_timeout(deps, channel, sequence)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Mul;

    use crate::execute::{transfer_ibc_token, handle_transfer_ibc_token_reply, receive_ibc_token, recover};
    use crate::ibc_lifecycle::receive_ack;
    use crate::proto::*;
    use crate::state::ibc::IBCTransfer;
    use crate::state::{TRANSFER_REPLY_STATE, TransferMsgReplyState, ibc, INFLIGHT_PACKETS, SEND_EXTERNAL_TOKENS_REPLY_STATE, RECOVERY_STATES};

    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_env, mock_info,
    };
    use cosmwasm_std::{
         Coin, Response, SubMsg, Uint128, to_binary, SubMsgResponse, SubMsgResult, coin, coins, BankMsg, CosmosMsg, Api,
    };
    use prost::Message;
    use schemars::_serde_json::json;
    use should_load::assignment::MapShouldLoad;
    
    #[test]
    fn instantiate_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("creator", &[]);
        let msg = InstantiateMsg {};

        // Instantiate the contract function
        let res = instantiate(deps.as_mut(), env.clone(), info.clone(), msg.clone());

        // Assert response
        assert_eq!(
            res.unwrap(),
            Response::new().add_attribute("method", "instantiate")
        );

        // Assert storage changes
        let state: Config = CONFIG.load(deps.as_ref().storage).unwrap();
        assert_eq!(state.owner, info.sender);
        assert_eq!(
            EXTERNAL_ADDRESSES
                .may_load(deps.as_ref().storage, "default".to_string())
                .unwrap(),
            Some("external_address".to_string())
        );
        assert_eq!(
            PORTS
                .may_load(deps.as_ref().storage, "default".to_string())
                .unwrap(),
            Some("transfer".to_string())
        );
        assert_eq!(
            CHANNELS
                .may_load(deps.as_ref().storage, "default".to_string())
                .unwrap(),
            Some(0)
        );
    }

    #[test]
    fn transfer_ibc_token_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(100, "token")]);
        let amount = Coin::new(50, "token");
        let port = "port".to_string();
        let channel = 0;
        let recipient = "recipient".to_string();

        // Execute the contract function
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");

        // Execute the contract function
        let res = transfer_ibc_token(deps.as_mut(), env.clone(), info.clone(), amount.clone(), port.clone(), channel.clone().to_string(), recipient.clone()).expect("ibc transfer should succeed");

        let memo_msg = serde_json_wasm::to_string(&json!({
            "ibc_callback": env.contract.address.to_string()
        })).unwrap();
        let source_channel = CHANNELS.should_load(deps.as_mut().storage, "default".to_string()).unwrap().to_string();
        let source_port = PORTS.should_load(deps.as_mut().storage, "default".to_string()).unwrap();
        let receiver = EXTERNAL_ADDRESSES.should_load(deps.as_mut().storage, "default".to_string()).unwrap();
        // build the transfer message
        let transfer_msg = MsgTransfer {
            source_port: source_port.clone(),
            source_channel: source_channel.clone(),
            token: Some(amount.clone().into()),
            sender: env.contract.address.to_string(),
            receiver: receiver.clone(),
            timeout_height: None,
            timeout_timestamp: None,
            memo: memo_msg,
        };
        // Assert response
        assert_eq!(
            res,
            Response::new()
                .set_data(to_binary(&transfer_msg).unwrap())
                .add_attribute("ibc_message", format!("{:?}", transfer_msg))
                .add_submessage(SubMsg::reply_on_success(
                    transfer_msg,
                    MsgReplyID::TransferIbc.repr(),
                ))
        );

        // Assert storage changes
        assert_eq!(
            TRANSFER_REPLY_STATE
                .may_load(deps.as_ref().storage)
                .unwrap(),
            Some(TransferMsgReplyState {
                channel_id: source_channel.clone().to_string(),
                to_address: receiver.clone().to_string(),
                amount: Uint128::from(50 as u128).u128(),
                denom: "token".to_string(),
                sender: info.sender.clone(),
            })
        );
    }

    #[test]
    fn transfer_ibc_token_test_insufficient_funds() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(50, "token")]);
        let amount = Coin::new(100, "token");
        let port = "port".to_string();
        let channel = 0;
        let recipient = "recipient".to_string();

        // Execute the contract function
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");

        // Execute the contract function
        let res = transfer_ibc_token(deps.as_mut(), env.clone(), info.clone(), amount.clone(), port.clone(), channel.clone().to_string(), recipient.clone());

        match res {
            Err(ContractError::NotEnoughFunds { .. }) => assert!(true),
            _ => panic!("Unexpected error"),
        }

        assert!(TRANSFER_REPLY_STATE.may_load(deps.as_ref().storage).unwrap().is_none())
    }

    #[test]
    fn handle_transfer_ibc_token_reply_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(50, "token")]);
        let amount = Coin::new(100, "token");
        // let port = "default".to_string();
        let channel_id = 0;
        let recipient = "default".to_string();

        // Execute the contract function
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");


        let state = TransferMsgReplyState {
            channel_id: channel_id.to_string(),
            to_address: recipient.to_string(),
            amount: amount.amount.u128(),
            denom: amount.denom.clone(),
            sender: info.clone().sender,
        };
        TRANSFER_REPLY_STATE.save(deps.as_mut().storage, &state).unwrap();

        // Create a mock reply with the expected response data
        let response_data = MsgTransferResponse {
            sequence: 1,
        };
        let mut response_data_buf = vec![];
        response_data.encode_raw(&mut response_data_buf);
        // Test wrong response encoding
        let reply = Reply {
            result: SubMsgResult::Ok(SubMsgResponse {
                data: Some(Binary::from(vec![0, 1, 2])),
                events: vec![],
            }),
            id: MsgReplyID::TransferIbc.repr(),
        };
        handle_transfer_ibc_token_reply(deps.as_mut(), reply).expect_err("ibc transfer reply wrong encoding should fail");
        
        let reply = Reply {
            result: SubMsgResult::Ok(SubMsgResponse {
                data: Some(Binary::from(response_data_buf)),
                events: vec![],
            }),
            id: MsgReplyID::TransferIbc.repr(),
        };

        // Execute the contract function
        let res = handle_transfer_ibc_token_reply(deps.as_mut(), reply).expect("ibc transfer reply should succeed");
        // Assert response
        assert_eq!(
            res,
            Response::new()
                .add_attribute("status", "ibc_message_created")
                .add_attribute("amount", "100")
                .add_attribute("denom", "token")
                .add_attribute("channel", channel_id.to_string())
                .add_attribute("receiver", recipient.to_string())
        );

        // Assert storage changes
        assert_eq!(
            INFLIGHT_PACKETS
                .load(deps.as_ref().storage, (&channel_id.to_string(), 1))
                .unwrap(),
            ibc::IBCTransfer {
                recovery_addr: info.sender,
                channel_id: channel_id.to_string(),
                sequence: 1,
                amount: amount.amount.u128(),
                denom: amount.denom.clone(),
                status: ibc::PacketLifecycleStatus::Sent,
            }
        );

        // Assert removal of TRANSFER_REPLY_STATE
        assert_eq!(
            TRANSFER_REPLY_STATE
                .may_load(deps.as_ref().storage)
                .unwrap(),
            None
        );
    }

    #[test]
    fn receive_ibc_token_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(50, "token")]);
        let amount = Coin::new(100, "token");
        // let port = "default".to_string();
        let channel_id = 0;
        let recipient = "default".to_string();

        // Instantiate the contract
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");

        let sequence = 1;

        let recovery = ibc::IBCTransfer {
            recovery_addr: info.clone().sender,
            channel_id: channel_id.to_string(),
            sequence,
            amount: amount.amount.u128(),
            denom: amount.denom.to_string(),
            status: ibc::PacketLifecycleStatus::AwaitingResponse,
        };

        INFLIGHT_PACKETS
            .save(deps.as_mut().storage, (&channel_id.to_string(), sequence), &recovery)
            .unwrap();

        // Create a mock MessageInfo with the necessary funds for the external address
        // Test failure with insufficient funds
        let mut external_info = mock_info(&recipient, &[Coin::new(50, "token")]);

        // Execute the contract function
        receive_ibc_token(deps.as_mut(), mock_env(), external_info.clone(), channel_id.to_string(), sequence).expect_err("receive ibc token should fail on insufficient funds");

        external_info.funds = vec![Coin::new(200, "token")];
        let res = receive_ibc_token(deps.as_mut(), mock_env(), external_info.clone(), channel_id.to_string(), sequence).expect("receive ibc token should succeed");
        
        let msg = BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(amount.amount.u128().mul(2 as u128), amount.denom),
        };
        // Assert response
        assert_eq!(
            res,
            Response::new()
                .add_submessage(SubMsg::reply_always(msg, MsgReplyID::SendAddr.repr()))
        );

        // Assert storage changes
        assert_eq!(
            SEND_EXTERNAL_TOKENS_REPLY_STATE
                .load(deps.as_ref().storage)
                .unwrap(),
            ibc::IBCTransfer {
                recovery_addr:info.sender,
                channel_id: channel_id.to_string(),
                sequence,
                amount: 200,
                denom: "token".to_string(),
                status: ibc::PacketLifecycleStatus::SendingExternalTokens,
            }
        );
    }

    #[test]
    fn recover_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(50, "token")]);
        let amount = Coin::new(100, "token");
        // let port = "default".to_string();
        let channel_id = 0;
        // let recipient = "default".to_string();

        // Instantiate the contract
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");

        let recovery_1 = ibc::IBCTransfer {
            recovery_addr: info.clone().sender,
            channel_id: channel_id.to_string(),
            sequence: 1,
            amount: amount.amount.u128(),
            denom: amount.denom.to_string(),
            status: ibc::PacketLifecycleStatus::SendingExternalTokensFailure,
        };
        let recovery_2 = ibc::IBCTransfer {
            recovery_addr: info.clone().sender,
            channel_id: 2.to_string(),
            sequence: 2,
            amount:amount.amount.u128() * 2,
            denom: amount.denom.to_string(),
            status: ibc::PacketLifecycleStatus::SendingExternalTokensFailure,
        };

        RECOVERY_STATES
            .save(deps.as_mut().storage, &info.sender, &vec![recovery_1, recovery_2])
            .unwrap();

        // Execute the contract function
        let res = recover(deps.as_mut(), info.clone().sender).expect("recover should succeed");

        // Assert response
        assert_eq!(
            res,
            Response::new().add_messages(vec![
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: info.clone().sender.to_string(),
                    amount: vec![coin(100, "token")]
                }),
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: info.clone().sender.to_string(),
                    amount: vec![coin(200, "token")]
                }),
            ])
        );

        // Assert storage changes
        assert_eq!(
            RECOVERY_STATES
                .may_load(deps.as_ref().storage, &info.sender)
                .unwrap(),
            None
        );

        // Try non-existing recovery
        let hacker = deps.api.addr_validate("unexisting_sender").unwrap();
        recover(deps.as_mut(), hacker).expect_err("recover should fail on non-existing recovery");
    }

    #[test]
    fn receive_ack_test() {
        let mut deps = mock_dependencies();

        let env = mock_env();
        let info = mock_info("sender", &[Coin::new(50, "token")]);
        let amount = Coin::new(100, "token");
        let sequence = 1;
        let channel_id = 0;

        // Instantiate the contract
        instantiate(deps.as_mut(), env.clone(), info.clone(), InstantiateMsg {  }).expect("contract instantiate fine");

        let inflight_packet = ibc::IBCTransfer {
            recovery_addr: info.clone().sender,
            channel_id: channel_id.to_string(),
            sequence,
            amount: amount.amount.u128(),
            denom: amount.denom.to_string(),
            status: ibc::PacketLifecycleStatus::Sent,
        };
        INFLIGHT_PACKETS
            .save(deps.as_mut().storage, (&channel_id.to_string(), sequence), &inflight_packet)
            .unwrap();

        // Execute the contract function
        let res = receive_ack(
            deps.as_mut(),
            channel_id.to_string(),
            sequence,
            "acknowledged".to_string(),
            true,
        ).expect("receive ack should succeed");

        // Assert response
        assert_eq!(
            res,
            Response::new().add_attribute("contract", "ibc_transfer").add_attribute("action", "receive_ack").add_attribute("msg", "packet successfully delivered")
        );

        // Assert storage changes
        assert_eq!(
            INFLIGHT_PACKETS
                .may_load(deps.as_ref().storage, (&channel_id.to_string(), sequence))
                .unwrap(),
            Some(ibc::IBCTransfer {
                recovery_addr: info.clone().sender,
                channel_id: channel_id.to_string(),
                sequence,
                amount: amount.amount.u128(),
                denom: amount.denom.to_string(),
                status: ibc::PacketLifecycleStatus::AwaitingResponse,
            })
        );

        // Test Failed ack
        let res = receive_ack(
            deps.as_mut(),
            channel_id.to_string(),
            sequence,
            "failed".to_string(),
            false,
        ).expect("receive ack should succeed");

        // Assert response
        assert_eq!(
            res,
            Response::new().add_attribute("contract", "ibc_transfer").add_attribute("action", "receive_ack").add_attribute("msg", "recovery stored").add_attribute("recovery_addr", info.clone().sender.to_string())
        );
         // Assert storage changes
        assert_eq!(
            RECOVERY_STATES
                .may_load(deps.as_ref().storage, &info.sender)
                .unwrap(),
            Some(vec![IBCTransfer {
                recovery_addr: info.clone().sender,
                channel_id: channel_id.to_string(),
                sequence,
                amount: amount.amount.u128(),
                denom: amount.denom.to_string(),
                status: ibc::PacketLifecycleStatus::AckFailure
            }])
        );

    }
}
