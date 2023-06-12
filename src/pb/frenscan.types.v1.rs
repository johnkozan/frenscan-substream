// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Transfers {
    #[prost(uint64, tag = "1")]
    pub block_number: u64,
    #[prost(int64, tag = "2")]
    pub block_timestamp: i64,
    #[prost(message, repeated, tag = "3")]
    pub value_transfers: ::prost::alloc::vec::Vec<ValueTransfer>,
    #[prost(message, repeated, tag = "4")]
    pub token_transfers: ::prost::alloc::vec::Vec<TokenTransfer>,
    #[prost(message, repeated, tag = "5")]
    pub call_traces: ::prost::alloc::vec::Vec<CallTraceRecord>,
    #[prost(message, repeated, tag = "6")]
    pub issued_token_transfers: ::prost::alloc::vec::Vec<TokenTransfer>,
}
#[derive(::serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TokenTransfer {
    /// The transaction hash that generated that transfer.
    #[prost(string, tag = "1")]
    pub tx_hash: ::prost::alloc::string::String,
    /// The index of the transaction in the block
    #[prost(uint32, tag = "2")]
    pub call_index: u32,
    /// The index of the log within the transaction's receipts of the block.
    #[prost(uint64, tag = "3")]
    pub log_index: u64,
    /// The person that received the transfer, might not be the same as the one that did initiated the
    /// transaction.
    #[prost(string, tag = "4")]
    pub from: ::prost::alloc::string::String,
    /// The person that received the transfer.
    #[prost(string, tag = "5")]
    pub to: ::prost::alloc::string::String,
    /// How many token were transferred in this transfer, will always be 1 in the case of ERC721.
    #[prost(string, tag = "6")]
    pub value: ::prost::alloc::string::String,
    /// Token Address
    #[prost(string, tag = "7")]
    pub token_address: ::prost::alloc::string::String,
    /// TokenID the identifier of the token for which the transfer is happening. Only
    /// available when `schema = ERC721` or `schema = ERC1155`. When `schema = ERC20`, the token id
    /// will be empty string "" as the contract itself is the token identifier.
    #[prost(string, tag = "8")]
    pub token_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ValueTransfer {
    /// The transaction or block hash that generated that transfer, depending on reason
    #[prost(string, tag = "1")]
    pub hash: ::prost::alloc::string::String,
    /// The index of the transaction in the block
    #[prost(uint32, tag = "2")]
    pub tx_index: u32,
    /// The index of the call in the transaction
    #[prost(uint32, tag = "3")]
    pub call_index: u32,
    /// The person that received the transfer, might not be the same as the one that did initiated the
    /// transaction.
    #[prost(string, tag = "4")]
    pub from: ::prost::alloc::string::String,
    /// The person that received the transfer.
    #[prost(string, tag = "5")]
    pub to: ::prost::alloc::string::String,
    /// How many token were transferred in this transfer, will always be 1 in the case of ERC721.
    #[prost(string, tag = "6")]
    pub value: ::prost::alloc::string::String,
    #[prost(string, tag = "7")]
    pub input: ::prost::alloc::string::String,
    #[prost(int32, tag = "8")]
    pub reason: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallTraceRecord {
    #[prost(string, tag = "1")]
    pub hash: ::prost::alloc::string::String,
    #[prost(uint32, tag = "2")]
    pub index: u32,
    /// Transaction calls in JSON
    #[prost(string, tag = "3")]
    pub traces: ::prost::alloc::string::String,
}
#[derive(::serde::Serialize)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallTrace {
    #[prost(uint32, tag = "1")]
    pub index: u32,
    #[prost(uint32, tag = "2")]
    pub parent_index: u32,
    #[prost(uint32, tag = "3")]
    pub depth: u32,
    #[prost(int32, tag = "4")]
    pub call_type: i32,
    #[prost(string, tag = "5")]
    pub caller: ::prost::alloc::string::String,
    #[prost(string, tag = "6")]
    pub address: ::prost::alloc::string::String,
    #[prost(string, tag = "7")]
    pub value: ::prost::alloc::string::String,
    #[prost(uint64, tag = "8")]
    pub gas_limit: u64,
    #[prost(uint64, tag = "9")]
    pub gas_consumed: u64,
    #[prost(string, tag = "10")]
    pub return_data: ::prost::alloc::string::String,
    #[prost(string, tag = "11")]
    pub input: ::prost::alloc::string::String,
    #[prost(bool, tag = "12")]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub executed_code: bool,
    #[prost(bool, tag = "13")]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suicide: bool,
    #[prost(bool, tag = "14")]
    pub status_failed: bool,
    #[prost(bool, tag = "15")]
    pub status_reverted: bool,
    #[prost(string, tag = "16")]
    pub failure_reason: ::prost::alloc::string::String,
    /// repeated string account_creations = 18; // # TODO
    #[prost(bool, tag = "17")]
    pub state_reverted: bool,
}
// @@protoc_insertion_point(module)
