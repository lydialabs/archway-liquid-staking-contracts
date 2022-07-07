use cosmwasm_std::StdError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.

    #[error("Different denominations in bonds: '{denom1}' vs. '{denom2}'")]
    DifferentBondDenom { denom1: String, denom2: String },

    #[error("No claims that can be released currently")]
    NothingToClaim {},

    #[error("User is not a liquidity provider to remove")]
    NothingToRemove{},

    #[error("Not enough liquidity to swap")]
    InsufficientLiquidity{},

    #[error("User adds zero lp token")]
    NothingToAdd {},

    #[error("User get no native token when swapping")]
    GainNothingWhenSwap {},

    #[error("User get no native token when removing liquidity")]
    GainNothingWhenRemove {},
}
