use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Uint128, Decimal, Coin};
use cw20::{Cw20ReceiveMsg};

use crate::linked_list::{NodeWithId, LinkedList};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// This is the liquid token contract address
    pub liquid_token_addr: String,
    /// This is the staking manager contract address
    pub staking_manager_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Add is called along with native tokens to add liquidity to the swap pool 
    Add {},
    /// Remove is used to remove liquidity provider from the pool and receive native token
    Remove {},
    /// Claim is called by liquidity provider to claim liquid token from swapping
    Claim {},
    /// Admin call this method to set up swap fee
    SetSwapFee { swap_fee: Uint128 },

    /// This accepts a properly-encoded ReceiveMsg from a cw20 contract (to process swapping request)
    Receive(Cw20ReceiveMsg),

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// ClaimableOf shows the number of liquid tokens the address can claim
    ClaimableOf { address: String },
    /// ConfigInfo shows the config of the contract
    ConfigInfo {},
    /// StatusInfo shows staking info of the contract
    StatusInfo {},
    /// Order book shows first 50 order in the swapping queue of the contract
    OrderBook {},
    /// OrderInfoOf shows status of the liquidity pool deposit of the address 
    OrderInfoOf { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Admin to change config
    pub owner: String,
    /// This is the denomination we can stake (and only one we accept for payments)
    pub bond_denom: String,
    /// Liquid token address
    pub liquid_token_addr: String,
    /// Staking manager contract address
    pub staking_manager_addr: String,
    /// Swap fee
    pub swap_fee: Uint128
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StatusResponse {
    /// issued is how many derivative tokens this contract has issued
    pub issued: Uint128,
    /// claims is how many tokens need to be reserved paying back those who unbonded
    pub claims: Uint128,
    /// available native token balance of this contract
    pub balance: Uint128,
    /// ratio of balance / issued (or how many native tokens that one derivative token is nominally worth)
    pub ratio: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OrderInfoOfResponse {
    /// issued is how many derivative tokens in the pool this address has 
    pub issued: Uint128,
    /// native is how many native tokens in the pool this address has
    pub native: Uint128,
    /// the block height shows when this address added native token to the pool
    pub height: u64,
    /// node_id is the id of adddress order in the linked-list
    pub node_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OrderBookResponse {
    pub state: LinkedList,
    pub queue: Vec<NodeWithId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingManagerQueryMsg {
    /// StatusInfo shows staking info of the contract
    StatusInfo {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingManagerStatusResponse {
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