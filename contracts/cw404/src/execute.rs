use cosmwasm_std::{
    to_json_binary, Binary, DepsMut, Env, MessageInfo, Response, StdResult, Storage, Uint128,
    WasmMsg,
};

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::{
    Cw20ReceiveMsg, ALLOWANCE, APPROVED_FOR_ALL, BALANCES, BASE_TOKEN_URI, DECIMALS, GET_APPROVED,
    LOCKED, MINTED, NAME, OWNED, OWNED_INDEX, OWNER, OWNER_OF, SYMBOL, TOTAL_SUPPLY, WHITELIST,
};

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let total_supply = msg.total_native_supply.u128() * ((10u128).pow(msg.decimals.into()));
    DECIMALS.save(deps.storage, &msg.decimals)?;
    TOTAL_SUPPLY.save(deps.storage, &Uint128::from(total_supply))?;
    MINTED.save(deps.storage, &Uint128::zero())?;
    NAME.save(deps.storage, &msg.name)?;
    SYMBOL.save(deps.storage, &msg.symbol)?;

    OWNER.save(deps.storage, &info.sender.to_string())?;

    BALANCES.save(deps.storage, &info.sender, &Uint128::from(total_supply))?;

    Ok(Response::new()
        .add_attribute("action", "mint")
        .add_attribute("to", info.sender.to_string())
        .add_attribute("amount", total_supply.to_string()))
}

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Approve {
            spender,
            token_id,
            expires: _,
        } => approve(deps, env, info, spender, token_id),
        ExecuteMsg::ApproveAll {
            operator,
            expires: _,
        } => approve_all(deps, env, info, operator),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires: _expires,
        } => approve(deps, env, info, spender, amount),
        ExecuteMsg::RevokeAll { operator } => revoke_all(deps, env, info, operator),
        // This is the default implementation in erc404
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => transfer_from(deps, env, info, owner, recipient, amount, None),
        // This is the default implementation in erc404
        ExecuteMsg::Transfer { recipient, amount } => transfer(
            deps,
            env,
            info.clone(),
            info.sender.to_string(),
            recipient,
            amount,
        ),
        // Added to ensure compatibility with cw721
        ExecuteMsg::TransferNft {
            recipient,
            token_id,
        } => transfer_from(
            deps,
            env,
            info.clone(),
            info.sender.to_string(),
            recipient,
            token_id,
            Some("transfer".to_string()),
        ),
        // Added to ensure compatibility with cw20
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => send(
            deps,
            env,
            info.clone(),
            info.sender.to_string(),
            contract,
            msg,
            amount,
        ),
        // Added to ensure compatibility with cw721
        ExecuteMsg::SendNft {
            contract,
            token_id,
            msg,
        } => send_nft(
            deps,
            env,
            info.clone(),
            info.sender.to_string(),
            contract,
            msg,
            token_id,
        ),
        // Additional feature added by dojo team to prevent accidental burning of CW721 tokens that a user may wish to keep (as cw20 transfers might burn tokens)
        ExecuteMsg::SetLock { token_id, state } => set_lock(deps, env, info, token_id, state),

        // Event functions
        ExecuteMsg::GenerateNftEvent {
            sender,
            recipient,
            token_id,
        } => generate_nft_event(deps, env, info.clone(), sender, recipient, token_id),
        ExecuteMsg::GenerateNftMintEvent {
            sender,
            recipient,
            token_id,
        } => generate_nft_mint_event(deps, env, info.clone(), sender, recipient, token_id),
        ExecuteMsg::GenerateNftBurnEvent { sender, token_id } => {
            generate_nft_burn_event(deps, env, info.clone(), sender, token_id)
        }

        // Auxillary functions
        ExecuteMsg::SetWhitelist { target, state } => set_whitelist(deps, env, info, target, state),
        ExecuteMsg::SetBaseTokenUri { uri } => set_base_token_uri(deps, env, info, uri),
    }
}

pub fn set_whitelist(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    target: String,
    state: bool,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if info.sender.to_string() != owner {
        return Err(ContractError::Unauthorized {});
    }

    // Prevents minting new NFTs by simply toggling the whitelist status.
    // This ensures that the capability to mint new tokens cannot be exploited
    // by reopen whitelist state.
    if state {
        let owned_list = OWNED
            .may_load(deps.storage, target.to_string())?
            .unwrap_or(vec![]);

        for _ in 0..owned_list.len() {
            _burn(deps.storage, env.clone(), target.to_string())?;
        }
    }

    WHITELIST.save(deps.storage, target.to_string(), &state)?;
    Ok(Response::new()
        .add_attribute("action", "set_whitelist")
        .add_attribute("address", target.to_string())
        .add_attribute("state", state.to_string()))
}

pub fn set_lock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    target: Uint128,
    state: bool,
) -> Result<Response, ContractError> {
    let owner_of = OWNER_OF
        .may_load(deps.storage, target.to_string())?
        .unwrap_or("".to_string());
    if info.sender.to_string() != owner_of {
        return Err(ContractError::Unauthorized {});
    }

    LOCKED.save(deps.storage, target.to_string(), &state)?;
    Ok(Response::new()
        .add_attribute("action", "set_lock")
        .add_attribute("target", target.to_string())
        .add_attribute("state", state.to_string()))
}

pub fn set_base_token_uri(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    uri: String,
) -> Result<Response, ContractError> {
    let owner = OWNER.load(deps.storage)?;
    if info.sender.to_string() != owner {
        return Err(ContractError::Unauthorized {});
    }

    BASE_TOKEN_URI.save(deps.storage, &uri.to_string())?;
    Ok(Response::new().add_attribute("action", "set_token_uri"))
}

fn transfer_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    to: String,
    amount_or_id: Uint128,
    event: Option<String>,
) -> Result<Response, ContractError> {
    let from_addr = deps.api.addr_validate(&from)?;
    let to_addr = deps.api.addr_validate(&to)?;

    let owner_of = OWNER_OF
        .may_load(deps.storage, amount_or_id.to_string())?
        .unwrap_or("".to_string());
    let minted = MINTED.load(deps.storage)?;
    let is_approved_for_all = APPROVED_FOR_ALL
        .may_load(deps.storage, (from.to_string(), info.sender.to_string()))?
        .unwrap_or(false);

    let get_approved = GET_APPROVED
        .may_load(deps.storage, amount_or_id.to_string())?
        .unwrap_or("".to_string());
    let unit = get_unit(deps.storage)?;

    if amount_or_id <= minted {
        if from != owner_of {
            return Err(ContractError::InvalidSender {});
        }

        if to == "" {
            return Err(ContractError::InvalidRecipient {});
        }

        if info.sender.to_string() != from
            && !is_approved_for_all
            && info.sender.to_string() != get_approved
        {
            return Err(ContractError::Unauthorized {});
        }

        // Prevents exploiting two different states of transferFrom can lead to a bug that allows minting
        // CW-721 tokens out of thin air through a whitelist
        if WHITELIST
            .may_load(deps.storage, to.clone())?
            .unwrap_or_default()
        {
            return Err(ContractError::InvalidRecipient {});
        }

        BALANCES.update(
            deps.storage,
            &from_addr,
            |balance: Option<Uint128>| -> StdResult<_> {
                Ok(balance.unwrap_or_default().checked_sub(unit)?)
            },
        )?;
        BALANCES.update(
            deps.storage,
            &to_addr,
            |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + unit) },
        )?;

        OWNER_OF.save(deps.storage, amount_or_id.to_string(), &to)?;

        GET_APPROVED.remove(deps.storage, amount_or_id.to_string());
        let mut vec_updated_id = OWNED
            .may_load(deps.storage, from.clone())?
            .unwrap_or(vec![]);

        let updated_id = vec_updated_id.get(vec_updated_id.len() - 1).unwrap();
        let owned_index = OWNED_INDEX
            .may_load(deps.storage, amount_or_id.to_string())?
            .unwrap_or(Uint128::zero());

        OWNED_INDEX.save(deps.storage, updated_id.to_string(), &owned_index)?;

        vec_updated_id[owned_index.u128() as usize] = updated_id.clone();
        vec_updated_id.pop();

        OWNED.save(deps.storage, from.clone(), &vec_updated_id)?;

        let mut to_owned = OWNED.may_load(deps.storage, to.clone())?.unwrap_or(vec![]);
        to_owned.push(amount_or_id);
        OWNED.save(deps.storage, to.clone(), &to_owned)?;

        OWNED_INDEX.save(
            deps.storage,
            amount_or_id.to_string(),
            &Uint128::from((to_owned.len() - 1) as u128),
        )?;
        Ok(Response::new()
            .add_message(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                msg: to_json_binary(&ExecuteMsg::GenerateNftEvent {
                    sender: from.clone(),
                    recipient: to.clone(),
                    token_id: amount_or_id,
                })?,
                funds: vec![],
            })
            .add_attribute("action", event.unwrap_or("transfer".to_string()))
            .add_attribute("from", from)
            .add_attribute("to", to)
            .add_attribute("amount", unit.to_string()))
    } else {
        let allowed = ALLOWANCE
            .may_load(deps.storage, (from.clone(), info.sender.to_string()))?
            .unwrap_or(Uint128::zero());
        if allowed != Uint128::MAX {
            ALLOWANCE.update(
                deps.storage,
                (from.clone(), info.sender.to_string()),
                |allow: Option<Uint128>| -> StdResult<_> {
                    Ok(allow.unwrap_or_default().checked_sub(amount_or_id)?)
                },
            )?;
        }

        let response = _transfer(
            deps,
            env,
            info.clone(),
            from,
            to,
            amount_or_id,
            event.unwrap_or("transfer_from".to_string()),
        )
        .unwrap();
        Ok(response.add_attribute("by", info.sender))
    }
}

fn approve(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    spender: String,
    amount_or_id: Uint128,
) -> Result<Response, ContractError> {
    let minted = MINTED.load(deps.storage)?;

    if amount_or_id <= minted && amount_or_id > Uint128::zero() {
        let owner = OWNER_OF
            .may_load(deps.storage, amount_or_id.to_string())?
            .unwrap_or("".to_string());

        let is_approved_for_all = APPROVED_FOR_ALL
            .may_load(deps.storage, (owner.to_string(), info.sender.to_string()))?
            .unwrap_or(false);
        if info.sender.to_string() != owner.to_string() && !is_approved_for_all {
            return Err(ContractError::Unauthorized {});
        }

        GET_APPROVED.save(deps.storage, amount_or_id.to_string(), &spender)?;
        Ok(Response::new()
            .add_attribute("action", "approve")
            .add_attribute("sender", owner.to_string())
            .add_attribute("spender", spender)
            .add_attribute("token_id", amount_or_id))
    } else {
        // ALLOWANCE
        ALLOWANCE.save(
            deps.storage,
            (info.sender.to_string(), spender.clone()),
            &amount_or_id,
        )?;

        Ok(Response::new()
            .add_attribute("action", "approve")
            .add_attribute("sender", info.sender)
            .add_attribute("spender", spender)
            .add_attribute("token_id", amount_or_id))
    }
}

fn approve_all(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    deps.api.addr_validate(&operator)?;

    APPROVED_FOR_ALL.save(
        deps.storage,
        (info.sender.to_string(), operator.clone()),
        &true,
    )?;

    Ok(Response::new()
        .add_attribute("action", "approve_all")
        .add_attribute("sender", info.sender)
        .add_attribute("operator", operator))
}

fn revoke_all(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    operator: String,
) -> Result<Response, ContractError> {
    deps.api.addr_validate(&operator)?;

    APPROVED_FOR_ALL.save(
        deps.storage,
        (info.sender.to_string(), operator.clone()),
        &false,
    )?;

    Ok(Response::new()
        .add_attribute("action", "revoke_all")
        .add_attribute("sender", info.sender)
        .add_attribute("operator", operator.to_string()))
}

fn transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    to: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    _transfer(deps, env, info, from, to, amount, "transfer".to_string())
}

fn send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    contract: String,
    msg: Binary,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let response = _transfer(
        deps,
        env,
        info.clone(),
        from,
        contract.clone(),
        amount,
        "send".to_string(),
    )
    .unwrap();
    Ok(response.add_message(
        Cw20ReceiveMsg {
            sender: info.sender.into(),
            amount,
            msg,
        }
        .into_cosmos_msg(contract)?,
    ))
}

fn send_nft(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    from: String,
    contract: String,
    msg: Binary,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let response = transfer_from(
        deps,
        env,
        info.clone(),
        from,
        contract.clone(),
        amount,
        Some("send".to_string()),
    )
    .unwrap();

    Ok(response.add_message(
        cw721::Cw721ReceiveMsg {
            sender: info.sender.into(),
            token_id: amount.to_string(),
            msg,
        }
        .into_cosmos_msg(contract)?,
    ))
}

fn get_unit(storage: &dyn Storage) -> Result<Uint128, ContractError> {
    let decimals = DECIMALS.load(storage)?;
    Ok(Uint128::from(10u128).pow(decimals.into()))
}

fn _transfer(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    from: String,
    to: String,
    amount: Uint128,
    event: String,
) -> Result<Response, ContractError> {
    let from_addr = deps.api.addr_validate(&from)?;
    let to_addr = deps.api.addr_validate(&to)?;
    let unit = get_unit(deps.storage)?;
    let balance_before_sender = BALANCES
        .may_load(deps.storage, &from_addr)?
        .unwrap_or_default();
    let balance_before_receiver = BALANCES
        .may_load(deps.storage, &to_addr)?
        .unwrap_or_default();

    BALANCES.update(
        deps.storage,
        &from_addr,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;

    BALANCES.update(
        deps.storage,
        &to_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let whitelist_from = WHITELIST
        .may_load(deps.storage, from.clone())?
        .unwrap_or_default();
    let whitelist_to = WHITELIST
        .may_load(deps.storage, to.clone())?
        .unwrap_or_default();

    let mut messages = vec![];
    // Skip burn for certain addresses to save gas
    if !whitelist_from {
        let tokens_to_burn = (balance_before_sender / unit)
            - (BALANCES
                .may_load(deps.storage, &from_addr)?
                .unwrap_or_default()
                / unit);
        for _i in 0..tokens_to_burn.u128() {
            let msg = _burn(deps.storage, env.clone(), from.clone())?;
            messages.push(msg);
        }
    }

    // Skip minting for certain addresses to save gas
    if !whitelist_to {
        let tokens_to_mint = (BALANCES
            .may_load(deps.storage, &to_addr)?
            .unwrap_or_default()
            / unit)
            - (balance_before_receiver / unit);
        for _i in 0..tokens_to_mint.u128() {
            let msg = _mint(deps.storage, env.clone(), to.clone())?;
            messages.push(msg);
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", event.to_string())
        .add_attribute("from", from)
        .add_attribute("to", to)
        .add_attribute("amount", amount))
}

fn _mint(storage: &mut dyn Storage, env: Env, to: String) -> Result<WasmMsg, ContractError> {
    if to == "" {
        return Err(ContractError::InvalidRecipient {});
    }

    let minted = MINTED.load(storage)?;
    let id = minted + Uint128::one();
    MINTED.save(storage, &id)?;

    let owner_of = OWNER_OF
        .may_load(storage, id.to_string())?
        .unwrap_or("".to_string());

    if owner_of != "" {
        return Err(ContractError::AlreadyExists {});
    }

    OWNER_OF.save(storage, id.to_string(), &to)?;

    let mut owned = OWNED.may_load(storage, to.clone())?.unwrap_or(vec![]);
    owned.push(id);
    OWNED.save(storage, to.clone(), &owned)?;
    OWNED_INDEX.save(
        storage,
        id.to_string(),
        &Uint128::from((owned.len() - 1) as u128),
    )?;

    Ok(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_json_binary(&ExecuteMsg::GenerateNftMintEvent {
            sender: env.contract.address.to_string(),
            recipient: to,
            token_id: id,
        })?,
        funds: vec![],
    })
}

fn _burn(storage: &mut dyn Storage, env: Env, from: String) -> Result<WasmMsg, ContractError> {
    if from == "" {
        return Err(ContractError::InvalidSender {});
    }

    let mut owned = OWNED.may_load(storage, from.clone())?.unwrap_or(vec![]);
    let id = owned[owned.len() - 1];
    owned.pop();
    OWNED.save(storage, from.clone(), &owned)?;
    OWNED_INDEX.remove(storage, id.to_string());
    OWNER_OF.remove(storage, id.to_string());
    GET_APPROVED.remove(storage, id.to_string());

    // Prevents burning if user has locked their token
    let locked = LOCKED.may_load(storage, id.to_string())?.unwrap_or(false);
    if locked {
        return Err(ContractError::PreventBurn {});
    }

    Ok(WasmMsg::Execute {
        contract_addr: env.contract.address.to_string(),
        msg: to_json_binary(&ExecuteMsg::GenerateNftBurnEvent {
            sender: from,
            token_id: id,
        })?,
        funds: vec![],
    })
}

/**
 * Additional functions to generate and emit events below
 */

pub fn generate_nft_event(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    recipient: String,
    token_id: Uint128,
) -> Result<Response, ContractError> {
    if info.sender.to_string() != env.contract.address.to_string() {
        return Err(ContractError::Unauthorized {});
    }

    let res = Response::new()
        .add_attribute("action", "transfer_nft")
        .add_attribute("sender", sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token_id", token_id);
    Ok(res)
}

pub fn generate_nft_mint_event(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    recipient: String,
    token_id: Uint128,
) -> Result<Response, ContractError> {
    if info.sender.to_string() != env.contract.address.to_string() {
        return Err(ContractError::Unauthorized {});
    }

    let res = Response::new()
        .add_attribute("action", "mint")
        .add_attribute("minter", sender)
        .add_attribute("owner", recipient)
        .add_attribute("token_id", token_id);
    Ok(res)
}

pub fn generate_nft_burn_event(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: String,
    token_id: Uint128,
) -> Result<Response, ContractError> {
    if info.sender.to_string() != env.contract.address.to_string() {
        return Err(ContractError::Unauthorized {});
    }

    let res = Response::new()
        .add_attribute("action", "burn")
        .add_attribute("sender", sender)
        .add_attribute("token_id", token_id);
    Ok(res)
}
