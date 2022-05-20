use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128, Decimal, Coin};
use cw20::{Cw20ReceiveMsg};

use crate::linked_list::{NodeWithId, LinkedList};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// This is the liquid token contract address
    // pub liquid_token_addr: Addr,
    /// This is the validator that all tokens will be bonded to
    pub validator: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Stake will stake and delegate all native tokens sent with the message and give back stkTokens
    Stake {},
    /// Claim is used to claim the amount of available native tokens that you previously "unstaked" 
    Claim {},
    /// Admin call this method to set up liquid token address 
    SetLiquidToken { address: Addr },

    /// This accepts a properly-encoded ReceiveMsg from a cw20 contract (to process unstake request)
    Receive(Cw20ReceiveMsg),

    _ProcessToken { balance_before: Uint128 },
    _PerformCheck {},
    _MintLiquidToken { receiver: Addr, native_amount: Uint128 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// ClaimableOf shows the number of native tokens the address can claim
    ClaimableOf { address: String },
    /// ConfigInfo shows the config of the contract
    ConfigInfo {},
    /// StatusInfo shows staking info of the contract
    StatusInfo {},
    /// UnstakingQueue shows first 50 nodes in the unstaking queue of the contract
    UnstakingQueue {},
    /// UnderUnstaking shows the total number of native tokens this address is waiting to be unstaked
    UnderUnstakingOf { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Admin to change config
    pub owner: String,
    /// This is the denomination we can stake (and only one we accept for payments)
    pub bond_denom: String,
    /// Liquid token address
    pub liquid_token_addr: String,
    /// All tokens are bonded to this validator
    /// FIXME: address validation doesn't work for validator addresses
    pub validator: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StatusResponse {
    /// issued is how many derivative tokens this contract has issued
    pub issued: Uint128,
    /// native is how many total supply of native tokens liquid token holders can withdraw
    pub native: Coin,
    /// unstakings is how many total native tokens in unstaking queue
    pub unstakings: Uint128,
    /// claims is how many tokens need to be reserved paying back those who unbonded
    pub claims: Uint128,
    /// bonded is how many native tokens exist bonded to the validator
    pub bonded: Uint128,
    /// available native token balance of this contract
    pub balance: Uint128,
    /// ratio of native / issued (or how many native tokens that one derivative token is nominally worth)
    pub ratio: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnstakingQueueResponse {
    pub state: LinkedList,
    pub queue: Vec<NodeWithId>,
}