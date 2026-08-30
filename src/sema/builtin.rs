// SPDX-License-Identifier: Apache-2.0

use super::ast::{
    ArrayLength, Builtin, Diagnostic, Expression, File, Function, Namespace, Parameter, StructType,
    Symbol, Type,
};
use super::diagnostics::Diagnostics;
use super::eval::eval_const_number;
use super::expression::{ExprContext, ResolveTo};
use super::symtable::Symtable;
use crate::sema::{
    ast::{RetrieveType, Tag, UserTypeDecl},
    expression::{function_call::evaluate_argument, resolve_expression::expression},
    namespace::ResolveTypeContext,
    statements::parameter_list_to_expr_list,
};
use crate::Target;
use num_bigint::BigInt;
use num_traits::One;
use once_cell::sync::Lazy;
use solang_parser::pt::CodeLocation;
use solang_parser::pt::{self, Identifier};
use std::path::PathBuf;

pub struct Prototype {
    pub builtin: Builtin,
    pub namespace: Option<&'static str>,
    pub method: Vec<Type>,
    pub name: &'static str,
    pub params: Vec<Type>,
    pub ret: Vec<Type>,
    pub target: Vec<Target>,
    pub doc: &'static str,
    // Can this function be called in constant context (e.g. hash functions)
    pub constant: bool,
}

// A list of all Solidity builtins functions
pub static BUILTIN_FUNCTIONS: Lazy<[Prototype; 29]> = Lazy::new(|| {
    [
        Prototype {
            builtin: Builtin::ExtendInstanceTtl,
            namespace: None,
            method: vec![],
            name: "extendInstanceTtl",  
            params: vec![Type::Uint(32), Type::Uint(32)],
            ret: vec![Type::Int(64)],
            target: vec![Target::Soroban],
            doc: "If the TTL for the current contract instance and code (if applicable) is below `threshold` ledgers, extend `live_until_ledger_seq` such that TTL == `extend_to`, where TTL is defined as live_until_ledger_seq - current ledger.",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Assert,
            namespace: None,
            method: vec![],
            name: "assert",
            params: vec![Type::Bool],
            ret: vec![Type::Void],
            target: vec![],
            doc: "Abort execution if argument evaluates to false",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Print,
            namespace: None,
            method: vec![],
            name: "print",
            params: vec![Type::String],
            ret: vec![Type::Void],
            target: vec![],
            doc: "log string for debugging purposes. Runs on development chain only",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Require,
            namespace: None,
            method: vec![],
            name: "require",
            params: vec![Type::Bool],
            ret: vec![Type::Void],
            target: vec![],
            doc: "Abort execution if argument evaluates to false",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Require,
            namespace: None,
            method: vec![],
            name: "require",
            params: vec![Type::Bool, Type::String],
            ret: vec![Type::Void],
            target: vec![],
            doc: "Abort execution if argument evaluates to false. Report string when aborting",
            constant: false,
        },
        Prototype {
            builtin: Builtin::SelfDestruct,
            namespace: None,
            method: vec![],
            name: "selfdestruct",
            params: vec![Type::Address(true)],
            ret: vec![Type::Unreachable],
            target: vec![Target::EVM, Target::default_polkadot()],
            doc: "Destroys current account and deposits any remaining balance to address",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Keccak256,
            namespace: None,
            method: vec![],
            name: "keccak256",
            params: vec![Type::DynamicBytes],
            ret: vec![Type::Bytes(32)],
            target: vec![],
            doc: "Calculates keccak256 hash",
            constant: true,
        },
        Prototype {
            builtin: Builtin::Ripemd160,
            namespace: None,
            method: vec![],
            name: "ripemd160",
            params: vec![Type::DynamicBytes],
            ret: vec![Type::Bytes(20)],
            target: vec![],
            doc: "Calculates ripemd hash",
            constant: true,
        },
        Prototype {
            builtin: Builtin::Sha256,
            namespace: None,
            method: vec![],
            name: "sha256",
            params: vec![Type::DynamicBytes],
            ret: vec![Type::Bytes(32)],
            target: vec![],
            doc: "Calculates sha256 hash",
            constant: true,
        },
        Prototype {
            builtin: Builtin::Blake2_128,
            namespace: None,
            method: vec![],
            name: "blake2_128",
            params: vec![Type::DynamicBytes],
            ret: vec![Type::Bytes(16)],
            target: vec![Target::default_polkadot()],
            doc: "Calculates blake2-128 hash",
            constant: true,
        },
        Prototype {
            builtin: Builtin::Blake2_256,
            namespace: None,
            method: vec![],
            name: "blake2_256",
            params: vec![Type::DynamicBytes],
            ret: vec![Type::Bytes(32)],
            target: vec![Target::default_polkadot()],
            doc: "Calculates blake2-256 hash",
            constant: true,
        },
        Prototype {
            builtin: Builtin::Gasleft,
            namespace: None,
            method: vec![],
            name: "gasleft",
            params: vec![],
            ret: vec![Type::Uint(64)],
            target: vec![Target::default_polkadot(), Target::EVM],
            doc: "Return remaining gas left in current call",
            constant: false,
        },
        Prototype {
            builtin: Builtin::BlockHash,
            namespace: None,
            method: vec![],
            name: "blockhash",
            params: vec![Type::Uint(64)],
            ret: vec![Type::Bytes(32)],
            target: vec![Target::EVM],
            doc: "Returns the block hash for given block number",
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiDecode,
            namespace: Some("abi"),
            method: vec![],
            name: "decode",
            params: vec![Type::DynamicBytes],
            ret: vec![],
            target: vec![],
            doc: "Abi decode byte array with the given types",
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiEncode,
            namespace: Some("abi"),
            method: vec![],
            name: "encode",
            params: vec![],
            ret: vec![],
            target: vec![],
            doc: "Abi encode given arguments",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiEncodePacked,
            namespace: Some("abi"),
            method: vec![],
            name: "encodePacked",
            params: vec![],
            ret: vec![],
            target: vec![],
            doc: "Abi encode given arguments using packed encoding",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiEncodeWithSelector,
            namespace: Some("abi"),
            method: vec![],
            name: "encodeWithSelector",
            params: vec![Type::FunctionSelector],
            ret: vec![],
            target: vec![],
            doc: "Abi encode given arguments with selector",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiEncodeWithSignature,
            namespace: Some("abi"),
            method: vec![],
            name: "encodeWithSignature",
            params: vec![Type::String],
            ret: vec![],
            target: vec![],
            doc: "Abi encode given arguments with function signature",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::AbiEncodeCall,
            namespace: Some("abi"),
            method: vec![],
            name: "encodeCall",
            params: vec![],
            ret: vec![],
            target: vec![],
            doc: "Abi encode given arguments with function signature",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::Gasprice,
            namespace: Some("tx"),
            method: vec![],
            name: "gasprice",
            params: vec![Type::Uint(64)],
            ret: vec![Type::Value],
            target: vec![],
            doc: "Calculate price of given gas units",
            constant: false,
        },
        Prototype {
            builtin: Builtin::MulMod,
            namespace: None,
            method: vec![],
            name: "mulmod",
            params: vec![Type::Uint(256), Type::Uint(256), Type::Uint(256)],
            ret: vec![Type::Uint(256)],
            target: vec![],
            doc: "Multiply first two arguments, and the modulo last argument. Does not overflow",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::AddMod,
            namespace: None,
            method: vec![],
            name: "addmod",
            params: vec![Type::Uint(256), Type::Uint(256), Type::Uint(256)],
            ret: vec![Type::Uint(256)],
            target: vec![],
            doc: "Add first two arguments, and the modulo last argument. Does not overflow",
            // it should be allowed in constant context, but we don't support that yet
            constant: false,
        },
        Prototype {
            builtin: Builtin::SignatureVerify,
            namespace: None,
            method: vec![],
            name: "signatureVerify",
            params: vec![Type::Address(false), Type::DynamicBytes, Type::DynamicBytes],
            ret: vec![Type::Bool],
            target: vec![Target::Solana],
            doc: "ed25519 signature verification",
            constant: false,
        },
        Prototype {
            builtin: Builtin::UserTypeWrap,
            namespace: None,
            method: vec![Type::UserType(0)],
            name: "wrap",
            params: vec![],
            ret: vec![Type::UserType(0)],
            target: vec![],
            doc: "wrap type into user defined type",
            constant: true,
        },
        Prototype {
            builtin: Builtin::UserTypeUnwrap,
            namespace: None,
            method: vec![Type::UserType(0)],
            name: "unwrap",
            params: vec![Type::UserType(0)],
            ret: vec![],
            target: vec![],
            doc: "unwrap user defined type",
            constant: true,
        },
        Prototype {
            builtin: Builtin::ECRecover,
            namespace: None,
            method: vec![],
            name: "ecrecover",
            params: vec![
                Type::Bytes(32),
                Type::Uint(8),
                Type::Bytes(32),
                Type::Bytes(32),
            ],
            ret: vec![Type::Address(false)],
            target: vec![Target::EVM],
            doc: "Recover the address associated with the public key from elliptic curve signature",
            constant: false,
        },
        Prototype {
            builtin: Builtin::StringConcat,
            namespace: Some("string"),
            method: vec![],
            name: "concat",
            params: vec![Type::String, Type::String],
            ret: vec![Type::String],
            target: vec![],
            doc: "Concatenate string",
            constant: true,
        },
        Prototype {
            builtin: Builtin::BytesConcat,
            namespace: Some("bytes"),
            method: vec![],
            name: "concat",
            params: vec![Type::DynamicBytes, Type::DynamicBytes],
            ret: vec![Type::DynamicBytes],
            target: vec![],
            doc: "Concatenate bytes",
            constant: true,
        },
        Prototype {
            builtin: Builtin::AuthAsCurrContract,
            namespace: Some("auth"),
            method: vec![],
            name: "authAsCurrContract",
            params: vec![],
            ret: vec![],
            target: vec![Target::Soroban],
            doc: "Authorizes sub-contract calls for the next contract call on behalf of the current contract.",
            constant: false,
        },
    ]
});

// A list of all Solidity builtins variables
pub static BUILTIN_VARIABLE: Lazy<[Prototype; 17]> = Lazy::new(|| {
    [
        Prototype {
            builtin: Builtin::BlockCoinbase,
            namespace: Some("block"),
            method: vec![],
            name: "coinbase",
            params: vec![],
            ret: vec![Type::Address(true)],
            target: vec![Target::EVM],
            doc: "The address of the current block miner",
            constant: false,
        },
        Prototype {
            builtin: Builtin::BlockDifficulty,
            namespace: Some("block"),
            method: vec![],
            name: "difficulty",
            params: vec![],
            ret: vec![Type::Uint(256)],
            target: vec![Target::EVM],
            doc: "The difficulty for current block",
            constant: false,
        },
        Prototype {
            builtin: Builtin::GasLimit,
            namespace: Some("block"),
            method: vec![],
            name: "gaslimit",
            params: vec![],
            ret: vec![Type::Uint(64)],
            target: vec![Target::EVM],
            doc: "The gas limit",
            constant: false,
        },
        Prototype {
            builtin: Builtin::BlockNumber,
            namespace: Some("block"),
            method: vec![],
            name: "number",
            params: vec![],
            ret: vec![Type::Uint(64)],
            target: vec![],
            doc: "Current block number",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Slot,
            namespace: Some("block"),
            method: vec![],
            name: "slot",
            params: vec![],
            ret: vec![Type::Uint(64)],
            target: vec![Target::Solana],
            doc: "Current slot number",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Timestamp,
            namespace: Some("block"),
            method: vec![],
            name: "timestamp",
            params: vec![],
            ret: vec![Type::Uint(64)],
            target: vec![],
            doc: "Current timestamp in unix epoch (seconds since 1970)",
            constant: false,
        },
        Prototype {
            builtin: Builtin::MinimumBalance,
            namespace: Some("block"),
            method: vec![],
            name: "minimum_balance",
            params: vec![],
            ret: vec![Type::Value],
            target: vec![Target::default_polkadot()],
            doc: "Minimum balance required for an account",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ChainId,
            namespace: Some("block"),
            method: vec![],
            name: "chainid",
            params: vec![],
            ret: vec![Type::Uint(256)],
            target: vec![Target::EVM],
            doc: "Current chain id",
            constant: false,
        },
        Prototype {
            builtin: Builtin::BaseFee,
            namespace: Some("block"),
            method: vec![],
            name: "basefee",
            params: vec![],
            ret: vec![Type::Uint(256)],
            target: vec![Target::EVM],
            doc: "Current block's base fee",
            constant: false,
        },
        Prototype {
            builtin: Builtin::PrevRandao,
            namespace: Some("block"),
            method: vec![],
            name: "prevrandao",
            params: vec![],
            ret: vec![Type::Uint(256)],
            target: vec![Target::EVM],
            doc: "Random number provided by the beacon chain",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Calldata,
            namespace: Some("msg"),
            method: vec![],
            name: "data",
            params: vec![],
            ret: vec![Type::DynamicBytes],
            target: vec![],
            doc: "Raw input bytes to current call",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Sender,
            namespace: Some("msg"),
            method: vec![],
            name: "sender",
            params: vec![],
            ret: vec![Type::Address(true)],
            target: vec![],
            constant: false,
            doc: "Address of caller",
        },
        Prototype {
            builtin: Builtin::Signature,
            namespace: Some("msg"),
            method: vec![],
            name: "sig",
            params: vec![],
            ret: vec![Type::FunctionSelector],
            target: vec![],
            doc: "Function selector for current call",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Value,
            namespace: Some("msg"),
            method: vec![],
            name: "value",
            params: vec![],
            ret: vec![Type::Value],
            target: vec![],
            doc: "Value sent with current call",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Gasprice,
            namespace: Some("tx"),
            method: vec![],
            name: "gasprice",
            params: vec![],
            ret: vec![Type::Value],
            target: vec![Target::default_polkadot(), Target::EVM],
            doc: "gas price for one gas unit",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Origin,
            namespace: Some("tx"),
            method: vec![],
            name: "origin",
            params: vec![],
            ret: vec![Type::Address(false)],
            target: vec![Target::EVM],
            doc: "Original address of sender current transaction",
            constant: false,
        },
        Prototype {
            builtin: Builtin::Accounts,
            namespace: Some("tx"),
            method: vec![],
            name: "accounts",
            params: vec![],
            ret: vec![Type::Array(
                Box::new(Type::Struct(StructType::AccountInfo)),
                vec![ArrayLength::Dynamic],
            )],
            target: vec![Target::Solana],
            doc: "Accounts passed into transaction",
            constant: false,
        },
    ]
});

// A list of all Solidity builtins methods
pub static BUILTIN_METHODS: Lazy<[Prototype; 29]> = Lazy::new(|| {
    [
        Prototype {
            builtin: Builtin::ExtendTtl,
            namespace: None,
            // FIXME: For now as a PoC, we are only supporting this method for type `uint64`
            method: vec![Type::StorageRef(false, Box::new(Type::Uint(64)))],
            name: "extendTtl",
            params: vec![Type::Uint(32), Type::Uint(32)], // Parameters `threshold` and `extend_to` of type `uint32`
            ret: vec![Type::Int(64)],
            target: vec![Target::Soroban],
            doc: "If the entry's TTL is below `threshold` ledgers, extend `live_until_ledger_seq` such that TTL == `extend_to`, where TTL is defined as live_until_ledger_seq - current ledger.",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt8,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt8",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(8)],
            target: vec![],
            doc: "Reads a signed 8-bit integer from the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt16LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt16LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(16)],
            target: vec![],
            doc: "Reads a signed 16-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt32LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt32LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(32)],
            target: vec![],
            doc: "Reads a signed 32-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt64LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt64LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(64)],
            target: vec![],
            doc: "Reads a signed 64-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt128LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt128LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(128)],
            target: vec![],
            doc: "Reads a signed 128-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadInt256LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readInt256LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Int(256)],
            target: vec![],
            doc: "Reads a signed 256-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint8,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint8",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(8)],
            target: vec![],
            doc: "Reads an unsigned 8-bit integer from the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint16LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint16LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(16)],
            target: vec![],
            doc: "Reads an unsigned 16-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint32LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint32LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(32)],
            target: vec![],
            doc: "Reads an unsigned 32-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint64LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint64LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(64)],
            target: vec![],
            doc: "Reads an unsigned 64-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint128LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint128LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(128)],
            target: vec![],
            doc: "Reads an unsigned 128-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadUint256LE,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readUint256LE",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Uint(256)],
            target: vec![],
            doc: "Reads an unsigned 256-bit integer from the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::ReadAddress,
            namespace: None,
            method: vec![Type::DynamicBytes, Type::Slice(Box::new(Type::Bytes(1)))],
            name: "readAddress",
            params: vec![Type::Uint(32)],
            ret: vec![Type::Address(false)],
            target: vec![],
            doc: "Reads an address from the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt8,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt8",
            params: vec![Type::Int(8), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 8-bit integer to the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt16LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt16LE",
            params: vec![Type::Int(16), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 16-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt32LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt32LE",
            params: vec![Type::Int(32), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 32-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt64LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt64LE",
            params: vec![Type::Int(64), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 64-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt128LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt128LE",
            params: vec![Type::Int(128), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 128-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteInt256LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeInt256LE",
            params: vec![Type::Int(256), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes a signed 256-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteUint16LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeUint16LE",
            params: vec![Type::Uint(16), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an unsigned 16-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteUint32LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeUint32LE",
            params: vec![Type::Uint(32), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an unsigned 32-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteUint64LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeUint64LE",
            params: vec![Type::Uint(64), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an unsigned 64-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteUint128LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeUint128LE",
            params: vec![Type::Uint(128), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an unsigned 128-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteUint256LE,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeUint256LE",
            params: vec![Type::Uint(256), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an unsigned 256-bit integer to the specified offset as little endian",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteAddress,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeAddress",
            params: vec![Type::Address(false), Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Writes an address to the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteString,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeString",
            params: vec![Type::String, Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Write the contents of a string (without its length) to the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::WriteBytes,
            namespace: None,
            method: vec![Type::DynamicBytes],
            name: "writeBytes",
            params: vec![Type::DynamicBytes, Type::Uint(32)],
            ret: vec![],
            target: vec![],
            doc: "Write the contents of a bytes array (without its length) to the specified offset",
            constant: false,
        },
        Prototype {
            builtin: Builtin::RequireAuth,
            namespace: None,
            method: vec![Type::Address(false), Type::StorageRef(false, Box::new(Type::Address(false)))],
            name: "requireAuth",
            params: vec![],
            ret: vec![],
            target: vec![Target::Soroban],
            doc: "Checks if the address has authorized the invocation of the current contract function with all the arguments of the invocation. Traps if the invocation hasn't been authorized.",
            constant: false,
        },
    ]
});

/// Does function call match builtin
pub fn is_builtin_call(namespace: Option<&str>, fname: &str, ns: &Namespace) -> bool {
    BUILTIN_FUNCTIONS.iter().any(|p| {
        p.name == fname
            && p.namespace == namespace
            && (p.target.is_empty() || p.target.contains(&ns.target))
    })
}

/// Get the prototype for a builtin. If the prototype has arguments, it is a function else
/// it is a variable.
pub fn get_prototype(builtin: Builtin) -> Option<&'static Prototype> {
    BUILTIN_FUNCTIONS
        .iter()
        .find(|p| p.builtin == builtin)
        .or_else(|| BUILTIN_VARIABLE.iter().find(|p| p.builtin == builtin))
        .or_else(|| BUILTIN_METHODS.iter().find(|p| p.builtin == builtin))
}

/// Does variable name match builtin
pub fn builtin_var(
    loc: &pt::Loc,
    namespace: Option<&str>,
    fname: &str,
    ns: &Namespace,
    diagnostics: &mut Diagnostics,
) -> Option<(Builtin, Type)> {
    if let Some(p) = BUILTIN_VARIABLE
        .iter()
        .find(|p| p.name == fname && p.namespace == namespace)
    {
        if p.target.is_empty() || p.target.contains(&ns.target) {
            if ns.target.is_polkadot() && p.builtin == Builtin::Gasprice {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    String::from(
                        "use the function 'tx.gasprice(gas)' in stead, as 'tx.gasprice' may round down to zero. See https://solang.readthedocs.io/en/latest/language/builtins.html#gasprice",
                    ),
                ));
            }
            if ns.target == Target::Solana && p.builtin == Builtin::Value {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    String::from(
                        "Solana Cross Program Invocation (CPI) cannot transfer native value. See https://solang.readthedocs.io/en/latest/language/functions.html#value_transfer",
                    ),
                ));
            }
            if ns.target == Target::Solana && p.builtin == Builtin::Sender {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    String::from(
                        "'msg.sender' is not available on Solana. See https://solang.readthedocs.io/en/latest/targets/solana.html#msg-sender-solana",
                    ),
                ));
            }
            return Some((p.builtin, p.ret[0].clone()));
        }
    }

    None
}

/// Does variable name match any builtin namespace
pub fn builtin_namespace(namespace: &str) -> bool {
    BUILTIN_VARIABLE
        .iter()
        .any(|p| p.namespace == Some(namespace))
}

/// Is name reserved for builtins
pub fn is_reserved(fname: &str) -> bool {
    if fname == "type" || fname == "super" || fname == "this" {
        return true;
    }

    let is_builtin_function = BUILTIN_FUNCTIONS.iter().any(|p| {
        (p.name == fname && p.namespace.is_none() && p.method.is_empty())
            || (p.namespace == Some(fname))
    });

    if is_builtin_function {
        return true;
    }

    BUILTIN_VARIABLE.iter().any(|p| {
        (p.name == fname && p.namespace.is_none() && p.method.is_empty())
            || (p.namespace == Some(fname))
    })
}

/// Resolve a builtin call
pub(super) fn resolve_call(
    loc: &pt::Loc,
    namespace: Option<&str>,
    id: &str,
    args: &[pt::Expression],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    let funcs = BUILTIN_FUNCTIONS
        .iter()
        .filter(|p| p.name == id && p.namespace == namespace && p.method.is_empty())
        .collect::<Vec<&Prototype>>();

    // try to resolve the arguments, give up if there are any errors
    if args.iter().fold(false, |acc, arg| {
        acc | expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown).is_err()
    }) {
        return Err(());
    }

    let mut call_diagnostics = Diagnostics::default();

    for func in &funcs {
        let mut candidate_diagnostics = Diagnostics::default();
        let mut cast_args = Vec::new();

        if context.constant && !func.constant {
            candidate_diagnostics.push(Diagnostic::cast_error(
                *loc,
                format!(
                    "cannot call function '{}' in constant expression",
                    func.name
                ),
            ));
        } else if func.params.len() != args.len() {
            candidate_diagnostics.push(Diagnostic::cast_error(
                *loc,
                format!(
                    "builtin function '{}' expects {} arguments, {} provided",
                    func.name,
                    func.params.len(),
                    args.len()
                ),
            ));
        } else {
            // check if arguments can be implicitly casted
            for (i, arg) in args.iter().enumerate() {
                let ty = func.params[i].clone();

                evaluate_argument(
                    arg,
                    context,
                    ns,
                    symtable,
                    &ty,
                    &mut candidate_diagnostics,
                    &mut cast_args,
                );
            }
        }

        if candidate_diagnostics.any_errors() {
            if funcs.len() != 1 {
                candidate_diagnostics.push(Diagnostic::error(
                    *loc,
                    "cannot find overloaded builtin which matches signature".into(),
                ));
            }
            call_diagnostics.extend(candidate_diagnostics);
        } else {
            // tx.gasprice(1) is a bad idea, just like tx.gasprice. Warn about this
            if ns.target.is_polkadot() && func.builtin == Builtin::Gasprice {
                if let Ok((_, val)) = eval_const_number(&cast_args[0], ns, diagnostics) {
                    if val == BigInt::one() {
                        diagnostics.push(Diagnostic::warning(
                            *loc,
                            String::from(
                                "the function call 'tx.gasprice(1)' may round down to zero. See https://solang.readthedocs.io/en/latest/language/builtins.html#gasprice",
                            ),
                        ));
                    }
                }
            }

            diagnostics.extend(candidate_diagnostics);

            return Ok(Expression::Builtin {
                loc: *loc,
                tys: func.ret.to_vec(),
                kind: func.builtin,
                args: cast_args,
            });
        }
    }

    diagnostics.extend(call_diagnostics);

    Err(())
}

/// Resolve a builtin namespace call. The takes the unresolved arguments, since it has
/// to handle the special case "abi.decode(foo, (int32, bool, address))" where the
/// second argument is a type list. The generic expression resolver cannot deal with
/// this. It is only used in for this specific call.
pub(super) fn resolve_namespace_call(
    loc: &pt::Loc,
    namespace: &str,
    name: &str,
    args: &[pt::Expression],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Expression, ()> {
    if name == "concat" {
        let (kind, ty) = match namespace {
            "string" => (Builtin::StringConcat, Type::String),
            "bytes" => (Builtin::BytesConcat, Type::DynamicBytes),
            _ => unreachable!(),
        };

        let mut resolved_args = Vec::new();

        for arg in args {
            let expr = expression(
                arg,
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Type(&ty),
            )?;

            resolved_args.push(expr.cast(loc, &ty, true, ns, diagnostics)?);
        }

        return Ok(Expression::Builtin {
            loc: *loc,
            tys: vec![ty],
            kind,
            args: resolved_args,
        });
    }

    if name == "authAsCurrContract" {
        let mut resolved_args = Vec::new();

        for arg in args {
            let expr = expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown)?;

            resolved_args.push(expr);
        }

        return Ok(Expression::Builtin {
            loc: *loc,
            tys: Vec::new(),
            kind: Builtin::AuthAsCurrContract,
            args: resolved_args,
        });
    }

    // The abi.* functions need special handling, others do not
    if namespace != "abi" && namespace != "string" {
        return resolve_call(
            loc,
            Some(namespace),
            name,
            args,
            context,
            ns,
            symtable,
            diagnostics,
        );
    }

    let builtin = match name {
        "decode" => Builtin::AbiDecode,
        "encode" => Builtin::AbiEncode,
        "encodePacked" => Builtin::AbiEncodePacked,
        "encodeWithSelector" => Builtin::AbiEncodeWithSelector,
        "encodeWithSignature" => Builtin::AbiEncodeWithSignature,
        "encodeCall" => Builtin::AbiEncodeCall,
        _ => unreachable!(),
    };

    if builtin == Builtin::AbiDecode {
        if args.len() != 2 {
            diagnostics.push(Diagnostic::error(
                *loc,
                format!("function expects {} arguments, {} provided", 2, args.len()),
            ));

            return Err(());
        }

        // first args
        let data = expression(
            &args[0],
            context,
            ns,
            symtable,
            diagnostics,
            ResolveTo::Type(&Type::DynamicBytes),
        )?
        .cast(&args[0].loc(), &Type::DynamicBytes, true, ns, diagnostics)?;

        let mut tys = Vec::new();
        let mut broken = false;

        let ty_exprs = parameter_list_to_expr_list(&args[1], diagnostics)?;

        if ty_exprs.is_empty() {
            tys.push(Type::Void);
        } else {
            for arg in ty_exprs {
                let ty = ns.resolve_type(
                    context.file_no,
                    context.contract_no,
                    ResolveTypeContext::None,
                    arg.strip_parentheses(),
                    diagnostics,
                )?;

                if ty.is_mapping() || ty.is_recursive(ns) {
                    diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("Invalid type '{}': mappings and recursive types cannot be abi decoded or encoded", ty.to_string(ns))
                ));
                    broken = true;
                }

                tys.push(ty);
            }
        }

        return if broken {
            Err(())
        } else {
            Ok(Expression::Builtin {
                loc: *loc,
                tys,
                kind: builtin,
                args: vec![data],
            })
        };
    }

    let mut resolved_args = Vec::new();
    let mut args_iter = args.iter();

    match builtin {
        Builtin::AbiEncodeWithSelector => {
            // first argument is selector
            if let Some(selector) = args_iter.next() {
                let selector = expression(
                    selector,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&Type::Bytes(4)),
                )?;

                resolved_args.insert(
                    0,
                    selector.cast(
                        &selector.loc(),
                        &Type::FunctionSelector,
                        true,
                        ns,
                        diagnostics,
                    )?,
                );
            } else {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!(
                        "function requires one 'bytes{}' selector argument",
                        ns.target.selector_length()
                    ),
                ));

                return Err(());
            }
        }
        Builtin::AbiEncodeCall => {
            if args.len() != 2 {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    format!("function expects {} arguments, {} provided", 2, args.len()),
                ));

                return Err(());
            }

            // first argument is function
            let function = expression(
                &args[0],
                context,
                ns,
                symtable,
                diagnostics,
                ResolveTo::Unknown,
            )?;

            let ty = function.ty();

            match function.cast(&function.loc(), ty.deref_any(), true, ns, diagnostics)? {
                Expression::ExternalFunction { function_no, .. }
                | Expression::InternalFunction { function_no, .. } => {
                    let func = &ns.functions[function_no];

                    if !func.is_public() {
                        diagnostics.push(Diagnostic::error_with_note(
                            function.loc(),
                            "function is not public or external".into(),
                            func.loc,
                            format!("definition of {}", func.id.name),
                        ));
                    }

                    let params = &func.params;

                    resolved_args.push(function);

                    let args = parameter_list_to_expr_list(&args[1], diagnostics)?;

                    if args.len() != params.len() {
                        diagnostics.push(Diagnostic::error_with_note(
                            *loc,
                            format!(
                                "function takes {} arguments, {} provided",
                                params.len(),
                                args.len()
                            ),
                            func.loc,
                            format!("definition of {}", func.id.name),
                        ));

                        return Err(());
                    }

                    for (arg_no, arg) in args.iter().enumerate() {
                        let ty = ns.functions[function_no].params[arg_no].ty.clone();

                        let mut expr = expression(
                            arg,
                            context,
                            ns,
                            symtable,
                            diagnostics,
                            ResolveTo::Type(&ty),
                        )?;

                        expr = expr.cast(&arg.loc(), &ty, true, ns, diagnostics)?;

                        // A string or hex literal should be encoded as a string
                        if let Expression::BytesLiteral { .. } = &expr {
                            expr = expr.cast(&arg.loc(), &Type::String, true, ns, diagnostics)?;
                        }

                        resolved_args.push(expr);
                    }

                    return Ok(Expression::Builtin {
                        loc: *loc,
                        tys: vec![Type::DynamicBytes],
                        kind: builtin,
                        args: resolved_args,
                    });
                }
                expr => {
                    diagnostics.push(Diagnostic::error(
                        *loc,
                        format!(
                            "first argument should be function, got '{}'",
                            expr.ty().to_string(ns)
                        ),
                    ));

                    return Err(());
                }
            }
        }
        Builtin::AbiEncodeWithSignature => {
            // first argument is signature
            if let Some(signature) = args_iter.next() {
                let signature = expression(
                    signature,
                    context,
                    ns,
                    symtable,
                    diagnostics,
                    ResolveTo::Type(&Type::String),
                )?;

                resolved_args.insert(
                    0,
                    signature.cast(&signature.loc(), &Type::String, true, ns, diagnostics)?,
                );
            } else {
                diagnostics.push(Diagnostic::error(
                    *loc,
                    "function requires one 'string' signature argument".to_string(),
                ));

                return Err(());
            }
        }
        _ => (),
    }

    for arg in args_iter {
        let mut expr = expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown)?;
        let ty = expr.ty();

        if expr.tys().len() == 1 && expr.tys()[0] == Type::Void {
            diagnostics.push(Diagnostic::error(
                arg.loc(),
                "cannot pass a void expression as an abi.encode* argument".to_string(),
            ));
            return Err(());
        }

        if ty.is_mapping() || ty.is_recursive(ns) {
            diagnostics.push(Diagnostic::error(
                arg.loc(),
                format!("Invalid type '{}': mappings and recursive types cannot be abi decoded or encoded", ty.to_string(ns)),
            ));

            return Err(());
        }

        expr = expr.cast(&arg.loc(), ty.deref_any(), true, ns, diagnostics)?;

        // A string or hex literal should be encoded as a string
        if let Expression::BytesLiteral { .. } = &expr {
            expr = expr.cast(&arg.loc(), &Type::String, true, ns, diagnostics)?;
        }

        resolved_args.push(expr);
    }

    Ok(Expression::Builtin {
        loc: *loc,
        tys: vec![Type::DynamicBytes],
        kind: builtin,
        args: resolved_args,
    })
}

/// Resolve a builtin call
pub(super) fn resolve_method_call(
    expr: &Expression,
    id: &pt::Identifier,
    args: &[pt::Expression],
    context: &mut ExprContext,
    ns: &mut Namespace,
    symtable: &mut Symtable,
    diagnostics: &mut Diagnostics,
) -> Result<Option<Expression>, ()> {
    let expr_ty = expr.ty();
    let deref_ty = expr_ty.deref_memory();
    let funcs: Vec<_> = BUILTIN_METHODS
        .iter()
        .filter(|func| func.name == id.name && func.method.contains(deref_ty))
        .collect();

    // try to resolve the arguments, give up if there are any errors
    if args.iter().fold(false, |acc, arg| {
        acc | expression(arg, context, ns, symtable, diagnostics, ResolveTo::Unknown).is_err()
    }) {
        return Err(());
    }

    let mut call_diagnostics = Diagnostics::default();

    for func in &funcs {
        let mut candidate_diagnostics = Diagnostics::default();
        let mut cast_args = Vec::new();

        if context.constant && !func.constant {
            candidate_diagnostics.push(Diagnostic::cast_error(
                id.loc,
                format!(
                    "cannot call function '{}' in constant expression",
                    func.name
                ),
            ));
        } else if func.params.len() != args.len() {
            candidate_diagnostics.push(Diagnostic::cast_error(
                id.loc,
                format!(
                    "builtin function '{}' expects {} arguments, {} provided",
                    func.name,
                    func.params.len(),
                    args.len()
                ),
            ));
        } else {
            // check if arguments can be implicitly casted
            for (i, arg) in args.iter().enumerate() {
                // we may have arguments that parameters
                let ty = func.params[i].clone();

                evaluate_argument(
                    arg,
                    context,
                    ns,
                    symtable,
                    &ty,
                    &mut candidate_diagnostics,
                    &mut cast_args,
                );
            }
        }

        if !candidate_diagnostics.any_errors() {
            cast_args.insert(
                0,
                expr.cast(&id.loc, deref_ty, true, ns, diagnostics).unwrap(),
            );

            let returns = if func.ret.is_empty() {
                vec![Type::Void]
            } else {
                func.ret.to_vec()
            };

            diagnostics.extend(candidate_diagnostics);

            return Ok(Some(Expression::Builtin {
                loc: id.loc,
                tys: returns,
                kind: func.builtin,
                args: cast_args,
            }));
        }

        call_diagnostics.extend(candidate_diagnostics);
    }

    if funcs.is_empty() {
        Ok(None)
    } else {
        diagnostics.extend(call_diagnostics);
        Err(())
    }
}

impl Namespace {
    pub fn add_solana_builtins(&mut self) {
        let file_no = self.files.len();

        self.files.push(File {
            path: PathBuf::from("solana"),
            line_starts: Vec::new(),
            cache_no: None,
            import_no: None,
        });

        let id = pt::Identifier {
            loc: pt::Loc::Builtin,
            name: String::from("AccountInfo"),
        };

        assert!(self.add_symbol(
            file_no,
            None,
            &id,
            Symbol::Struct(pt::Loc::Builtin, StructType::AccountInfo)
        ));

        let id = pt::Identifier {
            loc: pt::Loc::Builtin,
            name: String::from("AccountMeta"),
        };

        assert!(self.add_symbol(
            file_no,
            None,
            &id,
            Symbol::Struct(pt::Loc::Builtin, StructType::AccountMeta)
        ));

        let mut func = Function::new(
            pt::Loc::Builtin,
            pt::Loc::Builtin,
            pt::Identifier {
                name: "create_program_address".to_string(),
                loc: pt::Loc::Builtin,
            },
            None,
            Vec::new(),
            pt::FunctionTy::Function,
            Some(pt::Mutability::Pure(pt::Loc::Builtin)),
            pt::Visibility::Public(None),
            vec![
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Array(
                        Box::new(Type::Slice(Box::new(Type::Bytes(1)))),
                        vec![ArrayLength::AnyFixed],
                    ),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Address(false),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
            ],
            vec![Parameter {
                loc: pt::Loc::Builtin,
                id: None,
                ty: Type::Address(false),
                ty_loc: None,
                readonly: false,
                indexed: false,
                infinite_size: false,
                recursive: false,
                annotation: None,
            }],
            self,
        );

        func.has_body = true;

        let func_no = self.functions.len();
        let id = Identifier {
            name: func.id.name.to_owned(),
            loc: pt::Loc::Builtin,
        };

        self.functions.push(func);

        assert!(self.add_symbol(
            file_no,
            None,
            &id,
            Symbol::Function(vec![(pt::Loc::Builtin, func_no)])
        ));

        let mut func = Function::new(
            pt::Loc::Builtin,
            pt::Loc::Builtin,
            pt::Identifier {
                name: "try_find_program_address".to_string(),
                loc: pt::Loc::Builtin,
            },
            None,
            Vec::new(),
            pt::FunctionTy::Function,
            Some(pt::Mutability::Pure(pt::Loc::Builtin)),
            pt::Visibility::Public(None),
            vec![
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Array(
                        Box::new(Type::Slice(Box::new(Type::Bytes(1)))),
                        vec![ArrayLength::AnyFixed],
                    ),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Address(false),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
            ],
            vec![
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Address(false),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
                Parameter {
                    loc: pt::Loc::Builtin,
                    id: None,
                    ty: Type::Bytes(1),
                    ty_loc: None,
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                },
            ],
            self,
        );

        func.has_body = true;

        let func_no = self.functions.len();
        let id = Identifier {
            name: func.id.name.to_owned(),
            loc: pt::Loc::Builtin,
        };

        self.functions.push(func);

        assert!(self.add_symbol(
            file_no,
            None,
            &id,
            Symbol::Function(vec![(pt::Loc::Builtin, func_no)])
        ));
    }

    pub fn add_soroban_builtins(&mut self) {
        // TODO: add soroban builtins
    }
    pub fn add_polkadot_builtins(&mut self) {
        let loc = pt::Loc::Builtin;
        let identifier = |name: &str| Identifier {
            name: name.into(),
            loc,
        };

        let file_no = self.files.len();
        self.files.push(File {
            path: PathBuf::from("polkadot"),
            line_starts: Vec::new(),
            cache_no: None,
            import_no: None,
        });

        // The Hash type from ink primitives.
        let type_no = self.user_types.len();
        self.user_types.push(UserTypeDecl {
            tags: vec![Tag {
                loc,
                tag: "notice".into(),
                no: 0,
                value: "The Hash type from ink primitives".into(),
            }],
            loc,
            name: "Hash".into(),
            ty: Type::Bytes(32),
            contract: None,
        });

        let symbol = Symbol::UserType(loc, type_no);
        assert!(self.add_symbol(file_no, None, &identifier("Hash"), symbol));

        // Chain extensions
        for mut func in [
            Function::new(
                loc,
                loc,
                pt::Identifier {
                    name: "chain_extension".to_string(),
                    loc,
                },
                None,
                Vec::new(),
                pt::FunctionTy::Function,
                None,
                pt::Visibility::Public(Some(loc)),
                vec![
                    Parameter {
                        loc,
                        id: Some(identifier("id")),
                        ty: Type::Uint(32),
                        ty_loc: Some(loc),
                        readonly: false,
                        indexed: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    },
                    Parameter {
                        loc,
                        id: Some(identifier("input")),
                        ty: Type::DynamicBytes,
                        ty_loc: Some(loc),
                        readonly: false,
                        indexed: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    },
                ],
                vec![
                    Parameter {
                        loc,
                        id: Some(identifier("return_value")),
                        ty: Type::Uint(32),
                        ty_loc: Some(loc),
                        readonly: false,
                        indexed: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    },
                    Parameter {
                        loc,
                        id: Some(identifier("output")),
                        ty: Type::DynamicBytes,
                        ty_loc: Some(loc),
                        readonly: false,
                        indexed: false,
                        infinite_size: false,
                        recursive: false,
                        annotation: None,
                    },
                ],
                self,
            ),
            // is_contract API
            Function::new(
                loc,
                loc,
                pt::Identifier {
                    name: "is_contract".to_string(),
                    loc,
                },
                None,
                Vec::new(),
                pt::FunctionTy::Function,
                Some(pt::Mutability::View(loc)),
                pt::Visibility::Public(Some(loc)),
                vec![Parameter {
                    loc,
                    id: Some(identifier("address")),
                    ty: Type::Address(false),
                    ty_loc: Some(loc),
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                }],
                vec![Parameter {
                    loc,
                    id: Some(identifier("is_contract")),
                    ty: Type::Bool,
                    ty_loc: Some(loc),
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                }],
                self,
            ),
            // set_code_hash API
            Function::new(
                loc,
                loc,
                pt::Identifier {
                    name: "set_code_hash".to_string(),
                    loc,
                },
                None,
                Vec::new(),
                pt::FunctionTy::Function,
                None,
                pt::Visibility::Public(Some(loc)),
                vec![Parameter {
                    loc,
                    id: Some(identifier("code_hash_ptr")),
                    // FIXME: The hash length should be configurable
                    ty: Type::Array(Type::Uint(8).into(), vec![ArrayLength::Fixed(32.into())]),
                    ty_loc: Some(loc),
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                }],
                vec![Parameter {
                    loc,
                    id: Some(identifier("return_code")),
                    ty: Type::Uint(32),
                    ty_loc: Some(loc),
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                }],
                self,
            ),
            // caller_is_root API
            Function::new(
                loc,
                loc,
                pt::Identifier {
                    name: "caller_is_root".to_string(),
                    loc,
                },
                None,
                Vec::new(),
                pt::FunctionTy::Function,
                Some(pt::Mutability::View(loc)),
                pt::Visibility::Public(Some(loc)),
                vec![],
                vec![Parameter {
                    loc,
                    id: Some(identifier("caller_is_root")),
                    ty: Type::Bool,
                    ty_loc: Some(loc),
                    readonly: false,
                    indexed: false,
                    infinite_size: false,
                    recursive: false,
                    annotation: None,
                }],
                self,
            ),
        ] {
            func.has_body = true;
            let func_no = self.functions.len();
            let id = identifier(&func.id.name);
            self.functions.push(func);
            assert!(self.add_symbol(file_no, None, &id, Symbol::Function(vec![(loc, func_no)])));
        }
    }
}
