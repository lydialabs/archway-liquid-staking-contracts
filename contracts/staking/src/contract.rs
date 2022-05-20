#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coin, coins, to_binary, Addr, BankMsg, Binary, Decimal, Deps, DepsMut, 
    DistributionMsg, Env, MessageInfo, QuerierWrapper, QueryRequest, WasmQuery,
    Response, StakingMsg, StdError, StdResult, Uint128, WasmMsg
};

use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20Contract, Cw20ExecuteMsg, Cw20ReceiveMsg, 
    TokenInfoResponse, Cw20QueryMsg};

use crate::linked_list::{LinkedList, NodeWithId, node_update_value, linked_list, linked_list_read,
    linked_list_append, linked_list_remove_head, linked_list_get_list};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, ConfigResponse, StatusResponse, UnstakingQueueResponse, 
    InstantiateMsg, QueryMsg};
use crate::state::{ConfigInfo, Supply, CONFIG, TOTAL_SUPPLY, CLAIMABLE, UNDER_UNSTAKING};

const FALLBACK_RATIO: Decimal = Decimal::one();

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:liquid-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let linked_list_init = LinkedList {
        head_id: 0,
        tail_id: 0,
        length: 0
    };
    linked_list(deps.storage).save(&linked_list_init)?;

    let denom = deps.querier.query_bonded_denom()?;
    let config_init = ConfigInfo {
        owner: info.sender,
        bond_denom: denom,
        liquid_token_addr: Addr::unchecked("none"), // msg.liquid_token_addr,
        validator: msg.validator,
    };
    CONFIG.save(deps.storage, &config_init)?;

    // set supply to 0
    let supply_init = Supply::default();
    TOTAL_SUPPLY.save(deps.storage, &supply_init)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake {} => execute_stake(deps, env, info),
        ExecuteMsg::Claim {} => execute_claim(deps, info),
        ExecuteMsg::SetLiquidToken { address } => execute_set_liquid_token(deps, info, address),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::_ProcessToken { balance_before } => _process_token(deps, env, info, balance_before),
        ExecuteMsg::_PerformCheck {} => _perform_check(deps, env, info),
        ExecuteMsg::_MintLiquidToken { receiver, native_amount } => _mint_liquid_token(deps, env, info, receiver, native_amount),
    }
}

// process unstaking queue then stake remain available native token
pub fn _process_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    balance_before: Uint128,
) -> Result<Response, ContractError> {
    // only allow this contract to call itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    let zero_balance = Uint128::zero();
    // check how many available native token we have
    let config = CONFIG.load(deps.storage)?;
    let mut balance = deps
        .querier
        .query_balance(&env.contract.address, &config.bond_denom)?;
    let claimed_reward = balance.amount.checked_sub(balance_before).map_err(StdError::overflow)?;

    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    supply.native += claimed_reward;
    balance.amount = balance.amount.checked_sub(supply.claims).map_err(StdError::overflow)?;
    // process unstaking queue
    let unstaking_requests: Vec<NodeWithId> = linked_list_get_list(deps.storage, 50)?;
    for request in unstaking_requests {
        if balance.amount == zero_balance {
            break;
        }
        let payout: Uint128;
        if request.info.value <= balance.amount {
            payout = request.info.value;
            linked_list_remove_head(deps.storage)?;
        } else {
            payout = balance.amount;
            node_update_value(deps.storage, request.id, request.info.value.checked_sub(payout).map_err(StdError::overflow)?)?;
        }
        supply.unstakings = supply.unstakings.checked_sub(payout).map_err(StdError::overflow)?;
        balance.amount = balance.amount.checked_sub(payout).map_err(StdError::overflow)?;
        supply.claims += payout;
        CLAIMABLE.update(
            deps.storage,
            &request.info.receiver,
            |claimable: Option<Uint128>| -> StdResult<_> { Ok(claimable.unwrap_or_default() + payout) },
        )?;
        UNDER_UNSTAKING.update(
            deps.storage,
            &request.info.receiver,
            |unstaking: Option<Uint128>| -> StdResult<_> { Ok(unstaking.unwrap_or_default().checked_sub(payout)?) },
        )?;
    }
    let mut res = Response::new();
    // and bond remain available to the validator
    if supply.unstakings == zero_balance && balance.amount > zero_balance{
        res = res.add_message(StakingMsg::Delegate {
            validator: config.validator,
            amount: balance.clone(),
        })
    } else if supply.unstakings > zero_balance && balance.amount == zero_balance {
        // unbond if not enough available native token to process unstaking
        let bonded = get_bonded(&deps.querier, &env.contract.address)?;
        if bonded > supply.native {
            let unstake_amount = bonded.checked_sub(supply.native).map_err(StdError::overflow)?;
            res = res.add_message(StakingMsg::Undelegate {
                validator: config.validator,
                amount: coin(unstake_amount.u128(), &config.bond_denom),
            })
        }
    }
    TOTAL_SUPPLY.save(deps.storage, &supply)?;

    res = res
        .add_attribute("action", "_processToken")
        .add_attribute("bonded", balance.amount);
    Ok(res)
}

// claim staking reward, process withdraw queue, then stake available native token
pub fn _perform_check(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // only allow this contract to call itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    
    let config = CONFIG.load(deps.storage)?;
    let balance_before = deps
        .querier
        .query_balance(&env.contract.address, &config.bond_denom)?.amount;
    // claim reward then process available native token
    let mut res = Response::new();
    // claim staking rewards
    let bonded = get_bonded(&deps.querier, &env.contract.address)?;
    if bonded > Uint128::zero() {
        res = res.add_message(DistributionMsg::WithdrawDelegatorReward {
            validator: config.validator,
        })
    }
    // process unstaking queue and available native token
    let msg = to_binary(&ExecuteMsg::_ProcessToken { balance_before })?;
    res = res.add_message(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg,
            funds: vec![],
        })
        .add_attribute("action", "_performCheck")
        .add_attribute("balance", balance_before);
    Ok(res)
}

// get_bonded returns the total amount of delegations from contract
// it ensures they are all the same denom
fn get_bonded(querier: &QuerierWrapper, contract: &Addr) -> Result<Uint128, ContractError> {
    let bonds = querier.query_all_delegations(contract)?;
    if bonds.is_empty() {
        return Ok(Uint128::zero());
    }
    let denom = bonds[0].amount.denom.as_str();
    bonds.iter().fold(Ok(Uint128::zero()), |racc, d| {
        let acc = racc?;
        if d.amount.denom.as_str() != denom {
            Err(ContractError::DifferentBondDenom {
                denom1: denom.into(),
                denom2: d.amount.denom.to_string(),
            })
        } else {
            Ok(acc + d.amount.amount)
        }
    })
}

fn get_token_supply(querier: &QuerierWrapper, token_addr: Addr,) -> StdResult<Uint128> {
    let cw20_query_response: TokenInfoResponse =
       querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: token_addr.to_string(),
            msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
    }))?;

    Ok(cw20_query_response.total_supply)
 }

// mint new liquid token to native token sender
pub fn _mint_liquid_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receiver: Addr,
    native_amount: Uint128,
) -> Result<Response, ContractError> {
    // only allow this contract to call itself
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    let config = CONFIG.load(deps.storage)?;
    // calculate to_mint and update total supply
    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    let liquid_supply = get_token_supply(&deps.querier, config.liquid_token_addr.clone())?;
    let to_mint = if liquid_supply.is_zero() {
        FALLBACK_RATIO * native_amount
    } else {
        native_amount.multiply_ratio(liquid_supply, supply.native)
    };
    supply.native += native_amount;
    TOTAL_SUPPLY.save(deps.storage, &supply)?;

    let mut res = Response::new()
        .add_attribute("action", "_mintLiquidToken")
        .add_attribute("from", receiver.clone())
        .add_attribute("staked", native_amount)
        .add_attribute("minted", to_mint);

    // transfer cw20 liquid token to staker
    // Cw20Contract is a function helper that provides several queries and message builder.
    let cw20 = Cw20Contract(config.liquid_token_addr);
    // Build a cw20 transfer send msg, that send collected funds to target address
    let msg = cw20.call(Cw20ExecuteMsg::Mint {
        recipient: receiver.into_string(),
        amount: to_mint,
    })?;
    res = res.add_message(msg);
    
    Ok(res)
}

pub fn execute_stake(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // ensure we have the proper denom
    let config = CONFIG.load(deps.storage)?;
    // payment finds the proper coin (or throws an error)
    let payment = info
        .funds
        .iter()
        .find(|x| x.denom == config.bond_denom)
        .ok_or_else(|| ContractError::EmptyBalance {
            denom: config.bond_denom.clone(),
        })?;

    let contract_addr = env.contract.address;
    let msg1 = to_binary(&ExecuteMsg::_PerformCheck {})?;
    let msg2 = to_binary(&ExecuteMsg::_MintLiquidToken { receiver: info.sender, native_amount: payment.amount })?;
    
    let res = Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: msg1,
            funds: vec![],
        })
        .add_message(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: msg2,
            funds: vec![],
        });
    Ok(res)
}

pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let contract_addr = env.contract.address;
    let config = CONFIG.load(deps.storage)?;

    // burn liquid token
    let cw20 = Cw20Contract(config.liquid_token_addr.clone());
    // Build a cw20 transfer send msg, that send collected funds to target address
    let msg1 = cw20.call(Cw20ExecuteMsg::Burn {
        amount,
    })?;

    // put unstaker to unstaking queue, update info
    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    let liquid_supply = get_token_supply(&deps.querier, config.liquid_token_addr)?;
    let amount_to_unstake = amount.multiply_ratio(supply.native, liquid_supply);
    supply.native = supply.native.checked_sub(amount_to_unstake).map_err(StdError::overflow)?;
    supply.unstakings += amount_to_unstake;
    TOTAL_SUPPLY.save(deps.storage, &supply)?;
    linked_list_append(deps.storage, sender.clone(), amount_to_unstake, env.block.height)?;
    UNDER_UNSTAKING.update(
        deps.storage,
        &sender,
        |claimable: Option<Uint128>| -> StdResult<_> { Ok(claimable.unwrap_or_default() + amount_to_unstake) },
    )?;
    let msg2 = to_binary(&ExecuteMsg::_PerformCheck {})?;
    
    let res = Response::new()
        .add_message(msg1)
        .add_message(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: msg2,
            funds: vec![],
        })
        .add_attribute("action", "unstake")
        .add_attribute("from", sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // info.sender is the address of the cw20 contract (that re-sent this message).
    // wrapper.sender is the address of the user that requested the cw20 contract to send this.
    // This cannot be fully trusted (the cw20 contract can fake it), so only use it for actions
    // in the address's favor (like paying/bonding tokens, not withdrawls)

    let config = CONFIG.load(deps.storage)?;
    // only allow liquid token contract to call 
    if info.sender != config.liquid_token_addr {
        return Err(ContractError::Unauthorized {});
    }

    let api = deps.api;
    execute_unstake(deps, env, api.addr_validate(&wrapper.sender)?, wrapper.amount)
}

pub fn execute_claim(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let mut to_send:Uint128 = Uint128::zero();
    CLAIMABLE.update(
        deps.storage,
        &info.sender,
        |claimable: Option<Uint128>| -> StdResult<_> {
            to_send = claimable.unwrap_or_default();
            Ok(Uint128::zero()) 
        },
    )?;
    if to_send.is_zero() {
        return Err(ContractError::NothingToClaim {});
    }
    // update total supply (lower claim)
    TOTAL_SUPPLY.update(deps.storage, |mut supply| -> StdResult<_> {
        supply.claims = supply.claims.checked_sub(to_send)?;
        Ok(supply)
    })?;
    
    // transfer tokens to the sender
    let res = Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(to_send.u128(), config.bond_denom),
        })
        .add_attribute("action", "claim")
        .add_attribute("from", info.sender)
        .add_attribute("amount", to_send);
    Ok(res)
}

pub fn execute_set_liquid_token(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // only allow owner to call 
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    CONFIG.update(deps.storage, |mut config| -> StdResult<_> {
        config.liquid_token_addr = address.clone();
        Ok(config)
    })?;

    let res = Response::new()
        .add_attribute("action", "setLiquidToken")
        .add_attribute("from", info.sender)
        .add_attribute("address", address);
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ClaimableOf { address } => {
            to_binary(&query_claimable_of(deps, address)?)
        },
        QueryMsg::ConfigInfo {} => to_binary(&query_config(deps)?),
        QueryMsg::StatusInfo {} => to_binary(&query_status(deps, _env)?),
        QueryMsg::UnstakingQueue {} => to_binary(&query_unstaking_queue(deps)?),
        QueryMsg::UnderUnstakingOf { address } => {
            to_binary(&query_under_unstaking_of(deps, address)?)
        },
    }
}

pub fn query_claimable_of(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let claimable = CLAIMABLE
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance: claimable })
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    let res = ConfigResponse {
        owner: config.owner.to_string(),
        bond_denom: config.bond_denom,
        liquid_token_addr: config.liquid_token_addr.to_string(),
        validator: config.validator,
    };
    Ok(res)
}

pub fn query_status(deps: Deps, _env: Env) -> StdResult<StatusResponse> {
    let config = CONFIG.load(deps.storage)?;
    let supply = TOTAL_SUPPLY.load(deps.storage)?;

    let bonded = get_bonded(&deps.querier, &_env.contract.address).unwrap();
    let balance = deps
        .querier
        .query_balance(&_env.contract.address, &config.bond_denom)?;
    let liquid_supply = get_token_supply(&deps.querier, config.liquid_token_addr)?;

    let res = StatusResponse {
        issued: liquid_supply,
        native: coin(supply.native.u128(), &config.bond_denom),
        unstakings: supply.unstakings,
        claims: supply.claims,
        bonded: bonded,
        balance: balance.amount,
        ratio: if liquid_supply.is_zero() {
            FALLBACK_RATIO
        } else {
            Decimal::from_ratio(supply.native, liquid_supply)
        },
    };
    Ok(res)
}

pub fn query_unstaking_queue(deps: Deps) -> StdResult<UnstakingQueueResponse> {
    let state = linked_list_read(deps.storage).load()?;
    let unstaking_requests: Vec<NodeWithId> = linked_list_get_list(deps.storage, 50)?;

    let res = UnstakingQueueResponse {
        state,
        queue: unstaking_requests,
    };
    Ok(res)
}

pub fn query_under_unstaking_of(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let unstaking = UNDER_UNSTAKING
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance: unstaking })
}
