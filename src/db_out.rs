use crate::pb::frenscan::{CallTraceRecord, TokenTransfer, Transfers, ValueTransfer};
use std::collections::HashMap;
use substreams_database_change::pb::database::{table_change::Operation, DatabaseChanges};

pub fn transfers_to_database_changes(changes: &mut DatabaseChanges, transfers: Transfers) {
    push_create_value_transfers(
        changes,
        transfers.value_transfers,
        transfers.block_number,
        transfers.block_timestamp,
    );
    push_create_token_transfers(
        changes,
        transfers.token_transfers,
        transfers.block_number,
        transfers.block_timestamp,
    );
    push_create_issued_token_transfers(
        changes,
        transfers.issued_token_transfers,
        transfers.block_number,
        transfers.block_timestamp,
    );
    push_create_call_traces(changes, transfers.call_traces);
}

fn push_create_token_transfers(
    changes: &mut DatabaseChanges,
    transfers: Vec<TokenTransfer>,
    block_number: u64,
    block_timestamp: i64,
) {
    for transfer in transfers.iter() {
        let pk: HashMap<String, String> = HashMap::from([
            (String::from("tx_hash"), transfer.tx_hash.to_string()),
            (String::from("log_index"), transfer.log_index.to_string()),
        ]);

        changes
            .push_change_composite("token_transfers", pk, transfer.log_index, Operation::Create)
            .change("call_index", (None, transfer.call_index))
            .change("from_address", (None, &transfer.from.to_string()))
            .change("to_address", (None, &transfer.to.to_string()))
            .change("block_number", (None, block_number))
            .change("value", (None, &transfer.value))
            .change("token_address", (None, &transfer.token_address.to_string()))
            .change("token_id", (None, &transfer.token_id))
            .change("timestamp", (None, block_timestamp));
    }
}

fn push_create_issued_token_transfers(
    changes: &mut DatabaseChanges,
    transfers: Vec<TokenTransfer>,
    block_number: u64,
    block_timestamp: i64,
) {
    for transfer in transfers.iter() {
        let pk: HashMap<String, String> = HashMap::from([
            (String::from("tx_hash"), transfer.tx_hash.to_string()),
            (String::from("log_index"), transfer.log_index.to_string()),
        ]);

        changes
            .push_change_composite(
                "tokens_issued_transfers",
                pk,
                transfer.log_index,
                Operation::Create,
            )
            .change("call_index", (None, transfer.call_index))
            .change("from_address", (None, &transfer.from.to_string()))
            .change("to_address", (None, &transfer.to.to_string()))
            .change("block_number", (None, block_number))
            .change("value", (None, &transfer.value))
            .change("token_address", (None, &transfer.token_address.to_string()))
            .change("token_id", (None, &transfer.token_id))
            .change("timestamp", (None, block_timestamp));
    }
}

fn push_create_value_transfers(
    changes: &mut DatabaseChanges,
    transfers: Vec<ValueTransfer>,
    block_number: u64,
    block_timestamp: i64,
) {
    for transfer in transfers.iter() {
        let pk: HashMap<String, String> = HashMap::from([
            (String::from("hash"), transfer.hash.to_string()),
            (String::from("call_index"), transfer.call_index.to_string()),
        ]);

        changes
            .push_change_composite(
                "value_transfers",
                pk,
                transfer.call_index as u64,
                Operation::Create,
            )
            .change("tx_index", (None, transfer.tx_index))
            .change("from_address", (None, &transfer.from.to_string()))
            .change("to_address", (None, &transfer.to.to_string()))
            .change("block_number", (None, block_number))
            .change("value", (None, &transfer.value))
            .change("timestamp", (None, block_timestamp))
            .change("reason", (None, transfer.reason));
    }
}

fn push_create_call_traces(changes: &mut DatabaseChanges, call_traces: Vec<CallTraceRecord>) {
    for call in call_traces.iter() {
        let pk: HashMap<String, String> = HashMap::from([
            (String::from("tx_hash"), call.hash.to_string()),
            (String::from("index"), call.index.to_string()),
        ]);

        changes
            .push_change_composite("call_traces", pk, call.index as u64, Operation::Create)
            .change("trace", (None, &call.traces.to_string()));
    }
}
