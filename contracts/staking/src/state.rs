use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Delegation {
    pub validator: String,
    pub percentage: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigInfo {
    /// Admin to change config
    pub owner: Addr,
    /// This is the denomination we can stake (and only one we accept for payments)
    pub bond_denom: String,
    /// Liquid token address
    pub liquid_token_addr: Addr,
    /// Delegations preferences for a whitelist of validators, each validator has a delegation percentage
    pub delegations: Option<Vec<Delegation>>, 
}

/// Supply is dynamic and tracks the current supply of staked and ERC20 tokens.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct Supply {
    /// native is how many total supply of native tokens liquid token holders can withdraw
    pub native: Uint128,
    // unstakings is how many total native tokens in unstaking queue
    pub unstakings: Uint128,
    /// claims is how many tokens need to be reserved paying back those who unbonded
    pub claims: Uint128,
}

pub const CONFIG: Item<ConfigInfo> = Item::new("config");
pub const TOTAL_SUPPLY: Item<Supply> = Item::new("total_supply");
pub const CLAIMABLE: Map<&Addr, Uint128> = Map::new("claimable");
pub const UNDER_UNSTAKING: Map<&Addr, Uint128> = Map::new("under_unstaking");