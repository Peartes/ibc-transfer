# Finally writing a contract
## IBC transfers
Write a cosmwasm smart contract which has the capability to send IBC token transfers and receive IBC token transfers. The contract is supposed to be deployed on a cosmos sdk based chain which support x/wasm and you can assume that the chain has integrated the `Ibc-hooks` module by osmosis: https://github.com/osmosis-labs/osmosis/tree/main/x/ibc-hooks
The goal of our contract is to allow a user to IBC transfer tokens to some outside address via our smart contract. After this the outside address will IBC transfer 2 times the amount tokens back to our contract, calling the contract using ibc-hooks and the interface defined in the contract. The contract then sends the multiplied funds back to the initial user.
## You can make the following assumptions :
- The outside address always sends funds sequentially, The first user to send funds to the address, is the first to receive multiplied funds
- There are no raceconditions within blocks 

## Submission criteria for the contract to include below functionality
- Accepting tokens from a users
- Sending the tokens over IBC to a predetermined address, port and transfer
- Supporting adding new addresses, ports and channels as an admin where users can send funds to.
- Receiving funds from that address
- Returning the multiplied funds to the user
- appropriate tests covering sufficient cases.
- A small readme guide to understand the design and run the tests
