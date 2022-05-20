use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigInfo {
    /// Admin to change config
    pub owner: Addr,
    /// This is the denomination we can stake (and only one we accept for payments)
    pub bond_denom: String,
    /// Liquid token address
    pub liquid_token_addr: Addr,
    /// Staking manager contract address
    pub staking_manager_addr: Addr,
    /// Swap fee for liquidity provider
    pub swap_fee: Uint128
}

/// Supply is dynamic and tracks the current supply of staked and ERC20 tokens.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct Supply {
    /// issued is how many derivative tokens this contract has issued
    pub issued: Uint128,
    /// claims is how many tokens need to be reserved paying back those who unbonded
    pub claims: Uint128,
}

pub const CONFIG: Item<ConfigInfo> = Item::new("config");
pub const TOTAL_SUPPLY: Item<Supply> = Item::new("total_supply");
pub const CLAIMABLE: Map<&Addr, Uint128> = Map::new("claimable");
pub const QUEUE_ID: Map<&Addr, u64> = Map::new("queue_id");