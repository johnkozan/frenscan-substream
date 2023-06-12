mod abi;
#[path = "db_out.rs"]
mod db;
mod frensfile;
mod pb;
mod settings;

#[macro_use]
extern crate lazy_static;

use pb::frenscan::{CallTrace, CallTraceRecord, TokenTransfer, Transfers, ValueTransfer};
use settings::{TOKENS_ISSUED, TREASURY_ADDRESSES};

use substreams::errors::Error;
use substreams::prelude::*;
use substreams::{log, Hex};
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::{Event, NULL_ADDRESS};

use abi::erc1155::events::TransferBatch as ERC1155TransferBatchEvent;
use abi::erc1155::events::TransferSingle as ERC1155TransferSingleEvent;
use abi::erc20::events::Transfer as ERC20TransferEvent;
use abi::erc721::events::Transfer as ERC721TransferEvent;

use abi::weth::events::Deposit as WETHDepositEvent;
use abi::weth::events::Withdrawal as WETHWithdrawlEvent;

substreams_ethereum::init!();

/// Extracts transfers events from the contract(s)
#[substreams::handlers::map]
fn map_blocks(blk: eth::Block) -> Result<Transfers, substreams::errors::Error> {
    let mut value_transfers: Vec<ValueTransfer> = Vec::new();
    let mut token_transfers: Vec<TokenTransfer> = Vec::new();
    let mut call_traces: Vec<CallTraceRecord> = Vec::new();
    let mut issued_transfers: Vec<TokenTransfer> = Vec::new();

    // check for block rewards
    match new_value_transfer_block_rewards_from_block(&blk) {
        Some(transfers) => {
            value_transfers.extend(transfers);
        }
        None => {}
    }

    for trace in blk.transaction_traces.iter() {
        //Check for gas usage
        let calls = &trace.calls;
        let root_call = calls.into_iter().nth(0).unwrap();
        if TREASURY_ADDRESSES
            .iter()
            .any(|a| a.to_vec() == root_call.caller)
        {
            match new_gas_transfer_from_call(&trace, &root_call) {
                Some(transfer) => {
                    value_transfers.extend(transfer);
                }
                None => {}
            }
        }

        // Check calls
        for call in trace
            .calls
            .iter()
            .filter(|c| c.call_type != eth::CallType::Delegate as i32)
            .filter(|c| c.state_reverted == false)
        {
            // Check calls for value transfers
            if TREASURY_ADDRESSES
                .iter()
                .any(|a| a.to_vec() == call.caller || a.to_vec() == call.address)
            {
                match new_value_transfer_from_call(&trace, &call) {
                    Some(transfer) => {
                        value_transfers.push(transfer);
                    }
                    None => {}
                }
            }

            // Check logs for token transfers
            token_transfers.extend(get_erc_transfers_from_logs(
                &call.logs,
                &trace.hash,
                call.index,
            ));

            // Check logs for issued token transfers
            issued_transfers.extend(get_erc_issued_transfers_from_logs(
                &call.logs,
                &trace.hash,
                call.index,
            ));
        }
    }

    // Save call traces for transactions with transfers
    let mut tx_hashes: Vec<&String> = token_transfers.iter().map(|t| &t.tx_hash).collect();
    tx_hashes.extend(issued_transfers.iter().map(|t| &t.tx_hash));
    tx_hashes.extend(
        value_transfers
            .iter()
            .filter(|t| {
                let r = eth::balance_change::Reason::from_i32(t.reason).unwrap();
                r != eth::balance_change::Reason::RewardMineBlock
                    && r != eth::balance_change::Reason::RewardMineUncle
            })
            .map(|v| &v.hash),
    );
    tx_hashes.sort();
    tx_hashes.dedup();

    for trace in blk.transaction_traces.iter() {
        if tx_hashes
            .iter()
            .any(|&h| *h == Hex(&trace.hash).to_string())
        {
            call_traces.push(get_call_traces(trace));
        }
    }

    let block_timestamp: i64 = match blk.header {
        Some(header) => header.timestamp.unwrap().seconds,
        None => 0,
    };

    token_transfers.sort_by(|a, b| a.log_index.cmp(&b.log_index));
    value_transfers.sort_unstable_by_key(|x| (x.tx_index, x.call_index));

    Ok(Transfers {
        block_number: blk.number,
        block_timestamp: block_timestamp,
        value_transfers: value_transfers,
        token_transfers: token_transfers,
        issued_token_transfers: issued_transfers,
        call_traces: call_traces,
    })
}

#[substreams::handlers::map]
pub fn db_out(transfers: Transfers) -> Result<DatabaseChanges, Error> {
    let mut database_changes: DatabaseChanges = Default::default();

    db::transfers_to_database_changes(&mut database_changes, transfers);

    Ok(database_changes)
}

fn get_erc_transfers_from_logs<'a>(
    logs: &'a Vec<eth::Log>,
    hash: &'a Vec<u8>,
    call_index: u32,
) -> impl Iterator<Item = TokenTransfer> + 'a {
    logs.iter().flat_map(move |log| {
        if let Some(event) = ERC20TransferEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| {
                (a == &event.to[..20] || a == &event.from[..20]) && (&event.to != &event.from)
            }) {
                return vec![new_erc20_transfer(&hash, call_index, log, event)];
            }
        }

        if let Some(event) = ERC721TransferEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| {
                (a == &event.to[..20] || a == &event.from[..20]) && (&event.to != &event.from)
            }) {
                return vec![new_erc721_transfer(&hash, call_index, log, event)];
            }
        }

        if let Some(event) = ERC1155TransferSingleEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| {
                (a == &event.to[..20] || a == &event.from[..20]) && (&event.to != &event.from)
            }) {
                return vec![new_erc1155_single_transfer(&hash, call_index, log, event)];
            }
        }

        if let Some(event) = ERC1155TransferBatchEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| {
                (a == &event.to[..20] || a == &event.from[..20]) && (&event.to != &event.from)
            }) {
                return new_erc1155_batch_transfer(&hash, call_index, log, event);
            }
        }

        // WETH Deposit/Withdrawl
        if let Some(event) = WETHDepositEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| a == &event.dst[..20]) {
                return vec![new_weth_deposit(&hash, call_index, log, event)];
            }
        }
        if let Some(event) = WETHWithdrawlEvent::match_and_decode(log) {
            if TREASURY_ADDRESSES.iter().any(|&a| a == &event.src[..20]) {
                return vec![new_weth_withdrawal(&hash, call_index, log, event)];
            }
        }

        vec![]
    })
}

fn get_erc_issued_transfers_from_logs<'a>(
    logs: &'a Vec<eth::Log>,
    hash: &'a Vec<u8>,
    call_index: u32,
) -> impl Iterator<Item = TokenTransfer> + 'a {
    logs.iter()
        .filter(|l| {
            TOKENS_ISSUED
                .iter()
                .any(|a| a.token_address == &l.address[..20])
        })
        .flat_map(move |log| {
            if let Some(event) = ERC20TransferEvent::match_and_decode(log) {
                return vec![new_erc20_transfer(&hash, call_index, log, event)];
            }

            if let Some(event) = ERC721TransferEvent::match_and_decode(log) {
                return vec![new_erc721_transfer(&hash, call_index, log, event)];
            }

            if let Some(event) = ERC1155TransferSingleEvent::match_and_decode(log) {
                return vec![new_erc1155_single_transfer(&hash, call_index, log, event)];
            }

            if let Some(event) = ERC1155TransferBatchEvent::match_and_decode(log) {
                return new_erc1155_batch_transfer(&hash, call_index, log, event);
            }

            vec![]
        })
}

fn get_call_traces(trace: &eth::TransactionTrace) -> CallTraceRecord {
    let traces: Vec<CallTrace> = trace
        .calls
        .iter()
        .map(|t| CallTrace {
            index: t.index,
            parent_index: t.parent_index,
            depth: t.depth,
            call_type: t.call_type,
            caller: Hex(&t.caller).to_string(),
            address: Hex(&t.address).to_string(),
            value: bytes_to_hex(&t.value.as_ref().unwrap_or(&eth::BigInt::default()).bytes),
            gas_limit: t.gas_limit,
            gas_consumed: t.gas_consumed,
            return_data: Hex(&t.return_data).to_string(),
            input: Hex(&t.input).to_string(),
            executed_code: t.executed_code,
            suicide: t.suicide,
            status_failed: t.status_failed,
            status_reverted: t.status_reverted,
            failure_reason: Hex(&t.failure_reason).to_string(),
            state_reverted: t.state_reverted,
            //account_creations: Vec<, //Hex(t.account_creations).to_string(),
        })
        .collect::<_>();

    CallTraceRecord {
        hash: Hex(&trace.hash).to_string(),
        index: trace.index,
        traces: format!("'{}'", serde_json::to_string(&traces).unwrap().to_string()),
    }
}

//Check for POW block rewards
fn new_value_transfer_block_rewards_from_block(blk: &eth::Block) -> Option<Vec<ValueTransfer>> {
    let mut transfers: Vec<ValueTransfer> = Vec::new();
    let mut reward_value: BigInt = substreams::scalar::BigInt::zero();
    let mut reward_address = String::new();

    // Check for PoW block rewards
    for balance_change in blk
        .balance_changes
        .iter()
        .filter(|b| {
            b.reason == eth::balance_change::Reason::RewardMineBlock as i32
                || b.reason == eth::balance_change::Reason::RewardMineUncle as i32
                || b.reason == eth::balance_change::Reason::RewardTransactionFee as i32
                || b.reason == eth::balance_change::Reason::RewardTransactionFee as i32
                || b.reason == eth::balance_change::Reason::Withdrawal as i32
        })
        .filter(|b| TREASURY_ADDRESSES.iter().any(|a| a.to_vec() == b.address))
        .collect::<Vec<_>>()
    {
        let new_value = balance_change
            .new_value
            .as_ref()
            .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
            .unwrap_or(BigInt::zero());

        let old_value = balance_change
            .old_value
            .as_ref()
            .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
            .unwrap_or(BigInt::zero());

        match eth::balance_change::Reason::from_i32(balance_change.reason).unwrap() {
            eth::balance_change::Reason::Withdrawal => {
                let (_, value) = (old_value - new_value).to_bytes_be();
                transfers.push(ValueTransfer {
                    call_index: 0,
                    from: "".to_string(),
                    to: Hex(&balance_change.address).to_string(),
                    value: Hex(value).to_string(),
                    hash: Hex(&blk.hash).to_string(),
                    tx_index: 0,
                    input: "".to_string(),
                    reason: balance_change.reason,
                });
            }

            // Handle block rewards
            _ => {
                reward_value = reward_value + new_value - old_value;
                if reward_address.is_empty() {
                    reward_address = Hex(&balance_change.address).to_string();
                }
            }
        }
    }

    // Check Transactions for transaction rewards
    let mut reward_value: BigInt = substreams::scalar::BigInt::zero();
    let mut reward_address = String::new();

    for trace in blk.transaction_traces.iter() {
        let calls = &trace.calls;
        let root_call = calls.into_iter().nth(0).unwrap();

        for balance_change in root_call
            .balance_changes
            .iter()
            .filter(|b| TREASURY_ADDRESSES.iter().any(|a| a.to_vec() == b.address))
            .filter(|b| {
                eth::balance_change::Reason::from_i32(b.reason).unwrap()
                    == eth::balance_change::Reason::RewardTransactionFee
            })
            .collect::<Vec<_>>()
        {
            let new_value = balance_change
                .new_value
                .as_ref()
                .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
                .unwrap_or(BigInt::zero());

            let old_value = balance_change
                .old_value
                .as_ref()
                .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
                .unwrap_or(BigInt::zero());

            if reward_address.is_empty() {
                reward_address = Hex(&balance_change.address).to_string();
            }
            reward_value = reward_value + new_value - old_value;
        }
    }

    if reward_value > BigInt::zero() {
        let (_, value) = reward_value.to_bytes_be();
        transfers.push(ValueTransfer {
            call_index: 0,
            from: "".to_string(),
            to: reward_address,
            value: Hex(value).to_string(),
            hash: Hex(&blk.hash).to_string(),
            tx_index: 0,
            input: "".to_string(),
            reason: eth::balance_change::Reason::RewardTransactionFee as i32,
        });
    }

    if transfers.len() > 0 {
        return Some(transfers);
    }

    None
}

// Create Gas transactions
fn new_gas_transfer_from_call(
    trace: &eth::TransactionTrace,
    root_call: &eth::Call,
) -> Option<Vec<ValueTransfer>> {
    let mut transfers: Vec<ValueTransfer> = Vec::new();
    let mut gas_value: BigInt = substreams::scalar::BigInt::zero();
    let mut gas_address = String::new();

    for balance_change in root_call
        .balance_changes
        .iter()
        .filter(|b| TREASURY_ADDRESSES.iter().any(|a| a.to_vec() == b.address))
        .filter(|b| {
            b.reason != eth::balance_change::Reason::Transfer as i32
                && b.reason != eth::balance_change::Reason::RewardTransactionFee as i32
        })
        .collect::<Vec<_>>()
    {
        let new_value = balance_change
            .new_value
            .as_ref()
            .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
            .unwrap_or(BigInt::zero());

        let old_value = balance_change
            .old_value
            .as_ref()
            .map(|value| BigInt::from_unsigned_bytes_be(&value.bytes).into())
            .unwrap_or(BigInt::zero());

        match eth::balance_change::Reason::from_i32(balance_change.reason) {
            Some(reason) => {
                match reason {
                    // Gas:
                    eth::balance_change::Reason::GasBuy
                    | eth::balance_change::Reason::GasRefund => {
                        if gas_address.is_empty() {
                            gas_address = Hex(&balance_change.address).to_string();
                        }
                        gas_value = gas_value + new_value - old_value;
                    }

                    _ => {
                        // Not a Gas or Reward fee:
                        let mut to_addr: String = "".to_string();
                        let mut from_addr: String = "".to_string();

                        let value_change = new_value - old_value;

                        if value_change > BigInt::zero() {
                            to_addr = Hex(&balance_change.address).to_string();
                        } else {
                            from_addr = Hex(&balance_change.address).to_string();
                        }

                        let (_, val_bytes) = value_change.to_bytes_be();
                        transfers.push(ValueTransfer {
                            call_index: balance_change.ordinal as u32,
                            from: from_addr,
                            to: to_addr,
                            value: bytes_to_hex(&val_bytes),
                            hash: Hex(&trace.hash).to_string(),
                            tx_index: trace.index,
                            input: "".to_string(),
                            reason: reason as i32,
                        });
                    }
                }
            }
            None => {}
        }
    }

    // Gas transaction
    if gas_value != BigInt::zero() {
        let mut to_addr: String = "".to_string();
        let mut from_addr: String = "".to_string();
        let reason: eth::balance_change::Reason;

        if gas_value > BigInt::zero() {
            to_addr = gas_address;
            reason = eth::balance_change::Reason::GasRefund;
        } else {
            from_addr = gas_address;
            reason = eth::balance_change::Reason::GasBuy;
        }

        let (_, value) = gas_value.to_bytes_be();
        transfers.push(ValueTransfer {
            call_index: trace.end_ordinal as u32,
            from: from_addr,
            to: to_addr,
            value: Hex(value).to_string(),
            hash: Hex(&trace.hash).to_string(),
            tx_index: trace.index,
            input: "".to_string(),
            reason: reason as i32,
        });
    }

    Some(transfers)
}

fn new_value_transfer_from_call(
    trace: &eth::TransactionTrace,
    call: &eth::Call,
) -> Option<ValueTransfer> {
    let v = match &call.value {
        Some(_) => Hex(&call.value.as_ref().unwrap().bytes).to_string(),
        None => {
            return None;
        }
    };
    if v == "" {
        return None;
    }
    Some(ValueTransfer {
        call_index: call.index,
        from: Hex(&call.caller).to_string(),
        to: Hex(&call.address).to_string(),
        value: v,
        hash: Hex(&trace.hash).to_string(),
        tx_index: trace.index,
        input: Hex(&call.input).to_string(),
        reason: eth::balance_change::Reason::Transfer as i32,
    })
}

fn new_erc20_transfer(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: ERC20TransferEvent,
) -> TokenTransfer {
    let (_, val_bytes) = event.value.to_bytes_be();
    TokenTransfer {
        from: Hex(&event.from).to_string(),
        to: Hex(&event.to).to_string(),
        value: bytes_to_hex(&val_bytes),
        tx_hash: Hex(hash).to_string(),
        call_index: call_index,
        log_index: log.block_index as u64,
        token_address: Hex(&log.address).to_string(),
        token_id: "".to_string(),
    }
}

fn new_erc721_transfer(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: ERC721TransferEvent,
) -> TokenTransfer {
    TokenTransfer {
        from: Hex(&event.from).to_string(),
        to: Hex(&event.to).to_string(),
        value: "01".to_string(),
        tx_hash: Hex(hash).to_string(),
        call_index: call_index,
        log_index: log.block_index as u64,
        token_id: event.token_id.to_string(),
        token_address: Hex(&log.address).to_string(),
    }
}

fn new_erc1155_single_transfer(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: ERC1155TransferSingleEvent,
) -> TokenTransfer {
    new_erc1155_transfer(hash, call_index, log, event)
}

fn new_erc1155_batch_transfer(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: ERC1155TransferBatchEvent,
) -> Vec<TokenTransfer> {
    if event.ids.len() != event.values.len() {
        log::info!("There is a different count for ids ({}) and values ({}) in transaction {} for log at block index {}, ERC1155 spec says lenght should match, ignoring the log completely for now",
        event.ids.len(),
        event.values.len(),
        Hex(&hash).to_string(),
        log.index,
        );

        return vec![];
    }

    event
        .ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let (_, val_bytes) = event.values.get(i).unwrap().to_bytes_be();
            TokenTransfer {
                from: Hex(&event.from).to_string(),
                to: Hex(&event.to).to_string(),
                value: bytes_to_hex(&val_bytes),
                tx_hash: Hex(hash).to_string(),
                call_index: call_index,
                log_index: log.block_index as u64,
                token_address: Hex(&log.address).to_string(),
                token_id: id.to_string(),
            }
        })
        .collect()
}

fn new_erc1155_transfer(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: ERC1155TransferSingleEvent,
) -> TokenTransfer {
    let (_, val_bytes) = event.value.to_bytes_be();
    TokenTransfer {
        from: Hex(event.from).to_string(),
        to: Hex(event.to).to_string(),
        call_index: call_index,
        value: bytes_to_hex(&val_bytes),
        tx_hash: Hex(hash).to_string(),
        log_index: log.block_index as u64,
        token_address: Hex(&log.address).to_string(),
        token_id: event.id.to_string(),
    }
}

fn new_weth_deposit(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: WETHDepositEvent,
) -> TokenTransfer {
    let (_, val_bytes) = event.wad.to_bytes_be();
    TokenTransfer {
        from: Hex(NULL_ADDRESS).to_string(),
        to: Hex(&event.dst).to_string(),
        value: bytes_to_hex(&val_bytes),
        tx_hash: Hex(hash).to_string(),
        call_index: call_index,
        log_index: log.block_index as u64,
        token_address: Hex(&log.address).to_string(),
        token_id: "".to_string(),
    }
}

fn new_weth_withdrawal(
    hash: &[u8],
    call_index: u32,
    log: &eth::Log,
    event: WETHWithdrawlEvent,
) -> TokenTransfer {
    let (_, val_bytes) = event.wad.to_bytes_be();
    TokenTransfer {
        to: Hex(NULL_ADDRESS).to_string(),
        from: Hex(&event.src).to_string(),
        value: bytes_to_hex(&val_bytes),
        tx_hash: Hex(hash).to_string(),
        call_index: call_index,
        log_index: log.block_index as u64,
        token_address: Hex(&log.address).to_string(),
        token_id: "".to_string(),
    }
}

fn bytes_to_hex(val: &Vec<u8>) -> String {
    let v = Hex(&val).to_string();
    if v.chars().count() % 2 == 0 {
        return v;
    }
    format!("{}{}", "0", v)
}
