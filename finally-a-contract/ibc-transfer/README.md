# IBC Transfer Contract

This repository contains a smart contract written in CosmWasm that enables IBC token transfers. The contract allows users to send tokens to an external address via the smart contract and receive multiplied funds back. It assumes the underlying chain has integrated the `Ibc-hooks` module by Osmosis.

## Functionality

The contract provides the following functionality:

- Accepts tokens from users.
- Sends tokens over IBC to a predetermined address, port, and channels.
- Supports adding new addresses, ports, and channels as an admin for users to send funds to.
- Receives funds from the external address.
- Returns the multiplied funds to the user.

## Assumptions

The contract makes the following assumptions:

- The external address always sends funds sequentially, with the first user to send funds being the first to receive multiplied funds.

## Design Overview
The contract is designed to be deployed on a Cosmos SDK-based chain that supports x/wasm and has integrated the `Ibc-hooks` module. It is written in Rust using the CosmWasm framework.
The contract receives user funds and creates an IBCTransfer message according to [ics-20](https://github.com/cosmos/ibc/blob/main/spec/app/ics-020-fungible-token-transfer/README.md) specification and broadcasts the message to the IBC module. The IBC module sends the message to the external address over a pre-determined (port, channel) combination. 
The external address sends the funds (2x more than the original funds) back to the contract, which triggers the contract execution using the ibc-hooks module. 
The contract then returns the multiplied funds to the initial user.

## Contract States
The contract has the following states:

**Config**: Stores the admin address allowed to add new addresses, ports, and channels. Set to the address that deploys the contract.

**EXTERNAL ADDRESSES**: A map of address alias to the external address to send funds to. e.g 
``` JSON
    {"cosmos_hub": "cosmos1...","quasar": "quasar..."}
```

**CHANNELS**: A map of address alias to the channel to send funds to. e.g 
``` JSON
    {"cosmos_hub": "channel-0","quasar": "channel-1"}
```

**PORTS**: A map of address alias to the port to send funds to. e.g 
``` JSON
    {"cosmos_hub": "transfer","quasar": "movement"}
```

**TRANSFER_REPLY_STATE**: In order to keep context between sub-message calls which happen when we send the IBC transfer message, we store some details of the transfer message. This is done by storing a `TransferMsgReplyState` struct in the contract state defined as follows:
``` Rust
    pub struct TransferMsgReplyState {
        pub channel_id: String,
        pub to_address: String,
        pub amount: u128,
        pub denom: String,
        pub sender: Addr,
    }
```
This allows us to keep track of an ongoing transfer. The sender field is populated as the sender of the transfer tx

**INFLIGHT_PACKETS**: A map of (channel_id, sequence) to the `IBCTransfer` struct (an ibc transfer msg packet). This is used to keep track of the inflight packets. Whenever an IBC transfer is sent successfully, the packet is added to this map to keep track of all ongoing transfers in the contract. The `IBCTransfer` struct is defined as follows:
``` Rust
    pub struct IBCTransfer {
        pub channel_id: String,
        pub sequence: u64,
        pub denom: String,
        pub amount: u128,
        pub sender: Addr,
        pub receiver: Addr,
    }
```
When the reply from the ibc transfer sub-msg is returned which is of the form
``` Rust
    pub struct TransferMsgReply {
        pub sequence: u64,
    }
```
the packet details are stored in the `INFLIGHT_PACKETS` map using the (channel_id, sequence) key.
When the response from the IBC transfer msg is returned to the contract (from the external address) through the `ibc-hooks`, this map is used to retrieve (using the (channel_id, sequence)) the appropriate user to send the accompanying funds to 
The interface of the called in execute msg by the external address (sent as the body of the wasm execute msg defined below )
```JSON
    "ReceiveToken": {
        "channel": "String", // channel id of the initial transfer
        "sequence": "u64", // sequence number of the packet
    }
```

**RECOVERY_STATES**: This is used as a failsafe to enable users recover their funds from the contract in case of a failed transaction. This scenario could occur when a user sends funds to the contract and the contract is unable to send the funds to the external address either because of some encoding issue or even light client expiration. In this case, the packet is stored in this state to keep track of re-claimable funds then the user can call the `recover` function to recover their funds. The `RECOVERY_STATES` map is used to keep track of the recovery states. The key is the sender address (the sender who had originally initiated the tx) of the transfer packet and the value is the `IBCTransfer` struct defined above already.

**SEND_EXTERNAL_TOKENS_REPLY_STATE**: This state keeps context between cosmos Bank sub-msg used to transfer the funds returned from the external account to the appropriate user. Should the transfer fail (which is highly unlikely), this state is used to keep track of the particular tx. The funds are then moved into the recovery state already discussed to allow a user to re-try moving the funds again. This state is a bit redundant and with appropriate guarantees can be removed
## Getting Started

To get started with this contract, follow the steps below:

### Prerequisites

- Ensure you have a Cosmos SDK-based chain that supports x/wasm and has integrated the `Ibc-hooks` module.

### Installation

1. Clone this repository:

   ```shell
   git clone https://github.com/peartes/ibc-transfer.git
   ```

   ```shell
   cd ibc-transfer-contract/finally-a-contract
   cargo wasm
    ```

### Testing
To run the tests for the contract, execute the following command:
    
    ```shell
    cargo test
    ```

### Usage
To use the IBC transfer contract, you need to deploy it on your Cosmos SDK-based chain and interact with it using transactions. Here is an overview of the contract's usage:

- Deploy the contract on your chain.

- As an admin, configure the predetermined address, port, and channels for receiving funds from users.

- Users can now send tokens to the provided addresses, ports, and channels using the appropriate IBC transfer commands.

- The external address receives the funds and triggers the contract execution using the ibc-hooks module.

- The contract multiplies the received funds and returns them to the initial user.

### Contributors
Kehinde Faleye <kenny.fale.kf@gmail.com>
Please feel free to contribute to this project by submitting pull requests with improvements or additional features.

License
This project is licensed under the MIT License - see the LICENSE file for details.

