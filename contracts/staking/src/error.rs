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
    
    #[error("No {denom} tokens sent")]
    EmptyBalance { denom: String },

    #[error("Different denominations in bonds: '{denom1}' vs. '{denom2}'")]
    DifferentBondDenom { denom1: String, denom2: String },

    #[error("No claims that can be released currently")]
    NothingToClaim {},

    // #[error("Balance should be zero but: '{balance}'")]
    // BalanceShouldBeZero { balance: String },
}
