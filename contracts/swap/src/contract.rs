#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    coins, to_binary, Addr, BankMsg, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, 
    QueryRequest, WasmQuery, Response, StdError, StdResult, Uint128, 
};

use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20Contract, Cw20ExecuteMsg, Cw20ReceiveMsg, 
    };

use crate::linked_list::{LinkedList, NodeWithId, node_read, node_update_value, 
    linked_list, linked_list_read, linked_list_append, linked_list_remove_head, 
    linked_list_remove, linked_list_get_list};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, ConfigResponse, StatusResponse, InstantiateMsg, QueryMsg, 
    OrderInfoOfResponse, OrderBookResponse, StakingManagerQueryMsg, 
    StakingManagerStatusResponse};
use crate::state::{ConfigInfo, Supply, CONFIG, TOTAL_SUPPLY, CLAIMABLE, QUEUE_ID};

const FALLBACK_RATIO: Decimal = Decimal::one();

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:liquid-swap";
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
        liquid_token_addr: deps.api.addr_validate(&msg.liquid_token_addr)?,
        staking_manager_addr: deps.api.addr_validate(&msg.staking_manager_addr)?,
        swap_fee: Uint128::from(100u32),
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
        ExecuteMsg::Add {} => execute_add(deps, env, info),
        ExecuteMsg::Remove {} => execute_remove(deps, env, info),
        ExecuteMsg::Claim {} => execute_claim(deps, info),
        ExecuteMsg::SetSwapFee { swap_fee } => execute_set_swap_fee(deps, info, swap_fee),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
    }
}

fn get_ratio(numerator: Uint128, denominator: Uint128) -> Decimal {
    if denominator.is_zero() {
        FALLBACK_RATIO
    } else {
        Decimal::from_ratio(numerator, denominator)
    }
}

pub fn execute_add(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
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
    let balance = deps
        .querier
        .query_balance(&env.contract.address, &config.bond_denom)?;
    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    let mut cur_native = balance.amount.checked_sub(payment.amount).map_err(StdError::overflow)?;
    let mut res = Response::new();
    // withdraw all native token from contract when there is no lp token in the pool
    if !cur_native.is_zero() && supply.issued.is_zero() {
        res = res.add_message(BankMsg::Send {
            to_address: config.owner.to_string(),
            amount: coins(cur_native.u128(), config.bond_denom),
        });
        cur_native = Uint128::zero();
    }
    let mut new_node_value = payment.amount * get_ratio(supply.issued, cur_native);
    // update supply info
    supply.issued += new_node_value;
    TOTAL_SUPPLY.save(deps.storage, &supply)?;
    // update node id of user in the queue
    let old_node_id = QUEUE_ID.may_load(deps.storage, &info.sender)?.unwrap_or_default();
    if old_node_id > 0 {
        let old_node_key = &old_node_id.to_be_bytes();
        let old_node = node_read(deps.storage).load(old_node_key)?;
        new_node_value += old_node.value;
        linked_list_remove(deps.storage, old_node_id)?;
    }
    let new_node_id = linked_list_append(deps.storage, info.sender.clone(), new_node_value, env.block.height)?;
    QUEUE_ID.save(deps.storage, &info.sender, &new_node_id)?;
    
    res = res.add_attribute("action", "add")
        .add_attribute("from", info.sender)
        .add_attribute("amount", payment.amount);
    Ok(res)
}

pub fn execute_remove(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // ensure we have the proper denom
    let config = CONFIG.load(deps.storage)?;

    let node_id = QUEUE_ID.may_load(deps.storage, &info.sender)?.unwrap_or_default();
    if node_id == 0 {
        return Err(ContractError::NothingToRemove {});
    }

    let node_key = &node_id.to_be_bytes();
    let cur_node = node_read(deps.storage).load(node_key)?;
    linked_list_remove(deps.storage, node_id)?;
    QUEUE_ID.save(deps.storage, &info.sender, &0)?;
    let balance = deps
        .querier
        .query_balance(&env.contract.address, &config.bond_denom)?;
    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    let native_amount = cur_node.value.multiply_ratio(balance.amount, supply.issued);
    supply.issued = supply.issued.checked_sub(cur_node.value).map_err(StdError::overflow)?;
    TOTAL_SUPPLY.save(deps.storage, &supply)?;

    // transfer tokens to the sender
    let res = Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(native_amount.u128(), config.bond_denom),
        })
        .add_attribute("action", "remove")
        .add_attribute("from", info.sender)
        .add_attribute("amount", native_amount)
        .add_attribute("lp_amount", cur_node.value);
    Ok(res)
}

pub fn execute_claim(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let to_send = CLAIMABLE.may_load(deps.storage, &info.sender)?.unwrap_or_default();
    if to_send == Uint128::zero() {
        return Err(ContractError::NothingToClaim {});
    }
    CLAIMABLE.save(deps.storage, &info.sender, &Uint128::zero())?;
    
    TOTAL_SUPPLY.update(deps.storage, |mut supply| -> StdResult<_> {
        supply.claims = supply.claims.checked_sub(to_send)?;
        Ok(supply)
    })?;

    // transfer liquid token
    let cw20 = Cw20Contract(config.liquid_token_addr);
    // Build a cw20 transfer send msg, that send collected funds to target address
    let msg = cw20.call(Cw20ExecuteMsg::Transfer {
        recipient: info.sender.to_string(),
        amount: to_send,
    })?;
    
    // transfer tokens to the sender
    let res = Response::new()
        .add_message(msg)
        .add_attribute("action", "claim")
        .add_attribute("from", info.sender)
        .add_attribute("amount", to_send);
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
    execute_swap(deps, env, api.addr_validate(&wrapper.sender)?, wrapper.amount)
}

pub fn execute_swap(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let contract_addr = env.contract.address;
    let config = CONFIG.load(deps.storage)?;

    let swap_fee = amount.multiply_ratio(config.swap_fee, 10000u128);
    let order_liquid_token_value = amount.checked_sub(swap_fee).map_err(StdError::overflow)?;
    // get liquid -> native ratio
    let staking_query_msg: StakingManagerQueryMsg = StakingManagerQueryMsg::StatusInfo {};
    let staking_query_response: StakingManagerStatusResponse =
       deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.staking_manager_addr.to_string(),
            msg: to_binary(&staking_query_msg)?,
    }))?;
    let order_native_value = order_liquid_token_value * staking_query_response.ratio;
    let balance = deps
        .querier
        .query_balance(contract_addr, &config.bond_denom)?;
    let mut supply = TOTAL_SUPPLY.load(deps.storage)?;
    let order_lp_token_value = order_native_value * get_ratio(supply.issued, balance.amount);
    if order_lp_token_value > supply.issued {
        return Err(ContractError::InsufficientLiquidity {});
    }
    supply.issued = supply.issued.checked_sub(order_lp_token_value).map_err(StdError::overflow)?;
    supply.claims += amount;
    TOTAL_SUPPLY.save(deps.storage, &supply)?;
    
    let mut is_filled = false;
    let mut remain_lp_token = order_lp_token_value;
    while !is_filled {
        // Get next order from the queue
        let linked_list_info = linked_list_read(deps.storage).load()?;
        let counterparty_id = linked_list_info.head_id;
        let counterparty_key = &counterparty_id.to_be_bytes();
        let counterparty_order = node_read(deps.storage).load(counterparty_key)?;
        let counterparty_address = counterparty_order.receiver;
        let counterparty_lp_amount = counterparty_order.value;
        let mut counterparty_filled = false;
        // Perform match. Matched amount is up to order size
        let matched_lp = counterparty_lp_amount.min(remain_lp_token);
        remain_lp_token -= matched_lp;
        // Check for a full fill of the order
        if matched_lp == counterparty_lp_amount {
            counterparty_filled = true;
        }
        // Counterparty earns a proportional amount of order + fees
        let liquid_token_earning = amount.multiply_ratio(matched_lp, order_lp_token_value);
        CLAIMABLE.update(
            deps.storage,
            &counterparty_address,
            |claimable: Option<Uint128>| -> StdResult<_> { Ok(claimable.unwrap_or_default() + liquid_token_earning) },
        )?;
        
        if counterparty_filled {
            linked_list_remove_head(deps.storage)?;
            QUEUE_ID.save(deps.storage, &counterparty_address, &0)?;
        } else {
            let new_counterparty_value = counterparty_order.value.checked_sub(matched_lp).map_err(StdError::overflow)?;
            // counterparty_order.value = new_counterparty_value;
            node_update_value(deps.storage, counterparty_id, new_counterparty_value)?;
        }
        // If no more remaining lp token, the order is fully filled
        if remain_lp_token == Uint128::zero() {
            is_filled = true;
        }
    }

    // transfer native tokens to the sender
    let res = Response::new()
        .add_message(BankMsg::Send {
            to_address: sender.to_string(),
            amount: coins(order_native_value.u128(), config.bond_denom),
        })
        .add_attribute("action", "swap")
        .add_attribute("from", sender)
        .add_attribute("amount", order_native_value);
    Ok(res)
}

pub fn execute_set_swap_fee(
    deps: DepsMut,
    info: MessageInfo,
    swap_fee: Uint128,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    // only allow liquid token contract to call 
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    config.swap_fee = swap_fee.clone();
    CONFIG.save(deps.storage, &config)?;

    let res = Response::new()
        .add_attribute("action", "setSwapFee")
        .add_attribute("from", info.sender)
        .add_attribute("amount", swap_fee);
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
        QueryMsg::OrderBook {} => to_binary(&query_order_book(deps)?),
        QueryMsg::OrderInfoOf { address } => {
            to_binary(&query_order_info_of(deps, _env, address)?)
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
        staking_manager_addr: config.staking_manager_addr.to_string(),
        swap_fee: config.swap_fee,
    };
    Ok(res)
}

pub fn query_status(deps: Deps, _env: Env) -> StdResult<StatusResponse> {
    let config = CONFIG.load(deps.storage)?;
    let supply = TOTAL_SUPPLY.load(deps.storage)?;

    let balance = deps
        .querier
        .query_balance(&_env.contract.address, &config.bond_denom)?;

    let res = StatusResponse {
        issued: supply.issued,
        claims: supply.claims,
        balance: balance.amount,
        ratio: get_ratio(balance.amount, supply.issued),
    };
    Ok(res)
}

pub fn query_order_info_of(deps: Deps, _env: Env, address: String) -> StdResult<OrderInfoOfResponse> {
    let config = CONFIG.load(deps.storage)?;
    let supply = TOTAL_SUPPLY.load(deps.storage)?;

    let balance = deps
        .querier
        .query_balance(&_env.contract.address, &config.bond_denom)?;

    let address = deps.api.addr_validate(&address)?;
    let node_id = QUEUE_ID
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    let mut issued = Uint128::zero();
    let mut height = 0;
    if node_id > 0 {
        let node_key = &node_id.to_be_bytes();
        let cur_node = node_read(deps.storage).load(node_key)?;
        issued = cur_node.value;
        height = cur_node.height;
    }
    let native = issued * get_ratio(balance.amount, supply.issued);

    Ok(OrderInfoOfResponse { issued, native, height, node_id})
}

pub fn query_order_book(deps: Deps) -> StdResult<OrderBookResponse> {
    let state = linked_list_read(deps.storage).load()?;

    let unstaking_requests: Vec<NodeWithId> = linked_list_get_list(deps.storage, 50)?;

    let res = OrderBookResponse {
        state,
        queue: unstaking_requests,
    };
    Ok(res)
}
