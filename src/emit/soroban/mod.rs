// SPDX-License-Identifier: Apache-2.0

pub(super) mod target;
use crate::codegen::{cfg::ControlFlowGraph, HostFunctions, Options};

use crate::emit::cfg::emit_cfg;
use crate::{emit::Binary, sema::ast};
use funty::Fundamental;
use inkwell::{
    context::Context,
    module::{Linkage, Module},
    types::FunctionType,
};
use soroban_sdk::xdr::{
    Limited, Limits, ScEnvMetaEntry, ScEnvMetaEntryInterfaceVersion, ScSpecEntry,
    ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeDef, ScSpecTypeUdt, ScSpecTypeVec,
    ScSpecUdtStructFieldV0, ScSpecUdtStructV0, StringM, WriteXdr,
};

const SOROBAN_ENV_INTERFACE_VERSION: ScEnvMetaEntryInterfaceVersion =
    ScEnvMetaEntryInterfaceVersion {
        protocol: 23,
        pre_release: 0,
    };

impl HostFunctions {
    pub fn function_signature<'b>(&self, bin: &Binary<'b>) -> FunctionType<'b> {
        let ty = bin.context.i64_type();
        match self {
            HostFunctions::PutContractData => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),
            HostFunctions::GetContractData => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::HasContractData => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::DeleteContractData => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            // https://github.com/stellar/stellar-protocol/blob/2fdc77302715bc4a31a784aef1a797d466965024/core/cap-0046-03.md#ledger-host-functions-mod-l
            // ;; If the entry's TTL is below `threshold` ledgers, extend `live_until_ledger_seq` such that TTL == `extend_to`, where TTL is defined as live_until_ledger_seq - current ledger.
            // (func $extend_contract_data_ttl (param $k_val i64) (param $t_storage_type i64) (param $threshold_u32_val i64) (param $extend_to_u32_val i64) (result i64))
            HostFunctions::ExtendContractDataTtl => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            // ;; If the TTL for the current contract instance and code (if applicable) is below `threshold` ledgers, extend `live_until_ledger_seq` such that TTL == `extend_to`, where TTL is defined as live_until_ledger_seq - current ledger.
            // (func $extend_current_contract_instance_and_code_ttl (param $threshold_u32_val i64) (param $extend_to_u32_val i64) (result i64))
            HostFunctions::ExtendCurrentContractInstanceAndCodeTtl => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::LogFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            HostFunctions::SymbolNewFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::VectorNew => bin.context.i64_type().fn_type(&[], false),
            HostFunctions::BytesNew => bin.context.i64_type().fn_type(&[], false),
            HostFunctions::VecPopBack => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::VecGet => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::VecPut => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),
            HostFunctions::Call => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),
            HostFunctions::VectorNewFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::VecUnpackToLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),
            HostFunctions::ObjToU64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjFromU64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToI64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjFromI64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::RequireAuth => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::AuthAsCurrContract => {
                bin.context.i64_type().fn_type(&[ty.into()], false)
            }
            HostFunctions::UpdateCurrentContractWasm => {
                bin.context.i64_type().fn_type(&[ty.into()], false)
            }
            HostFunctions::MapNewFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),

            HostFunctions::MapNew => bin.context.i64_type().fn_type(&[], false),

            HostFunctions::MapPut => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),

            HostFunctions::MapGet => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::MapDel => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::MapHas => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::VecPushBack => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),

            HostFunctions::VecLen => bin.context.i64_type().fn_type(&[ty.into()], false),

            HostFunctions::StringNewFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::StrKeyToAddr => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::GetLedgerTimestamp => bin.context.i64_type().fn_type(&[], false),
            HostFunctions::GetLedgerSequence => bin.context.i64_type().fn_type(&[], false),
            HostFunctions::GetCurrentContractAddress => bin.context.i64_type().fn_type(&[], false),
            HostFunctions::ObjToI128Lo64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToI128Hi64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToU128Lo64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToU128Hi64 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjFromI128Pieces => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::ObjFromU128Pieces => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::ObjToU256LoLo => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToU256LoHi => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToU256HiLo => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToU256HiHi => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjFromU256Pieces => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            HostFunctions::ObjToI256LoLo => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToI256LoHi => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToI256HiLo => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjToI256HiHi => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ObjFromI256Pieces => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            HostFunctions::BytesNewFromLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::BytesLen => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::BytesCopyToLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            HostFunctions::BytesGet => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::BytesPut => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into()], false),
            HostFunctions::BytesPush => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::BytesPop => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::StringLen => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::StringCopyToLinearMemory => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into(), ty.into(), ty.into()], false),
            HostFunctions::ContractEvent => bin
                .context
                .i64_type()
                .fn_type(&[ty.into(), ty.into()], false),
            HostFunctions::ComputeHashSha256 => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::ComputeHashKeccak256 => {
                bin.context.i64_type().fn_type(&[ty.into()], false)
            }
            HostFunctions::SerializeToBytes => bin.context.i64_type().fn_type(&[ty.into()], false),
            HostFunctions::DeserializeFromBytes => {
                bin.context.i64_type().fn_type(&[ty.into()], false)
            }
        }
    }
}

pub struct SorobanTarget;

impl SorobanTarget {
    fn type_to_spec(ty: &ast::Type, ns: &ast::Namespace) -> ScSpecTypeDef {
        match ty {
            ast::Type::Ref(inner) | ast::Type::SorobanHandle(inner) => {
                Self::type_to_spec(inner, ns)
            }
            ast::Type::Array(elem, _) => ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
                element_type: Box::new(Self::type_to_spec(elem, ns)),
            })),
            ast::Type::Uint(32) => ScSpecTypeDef::U32,
            ast::Type::Int(32) => ScSpecTypeDef::I32,
            ast::Type::Enum(_) => ScSpecTypeDef::U32,
            ast::Type::Uint(64) => ScSpecTypeDef::U64,
            ast::Type::Int(64) => ScSpecTypeDef::I64,
            ast::Type::Int(128) => ScSpecTypeDef::I128,
            ast::Type::Uint(128) => ScSpecTypeDef::U128,
            ast::Type::Int(256) => ScSpecTypeDef::I256,
            ast::Type::Uint(256) => ScSpecTypeDef::U256,
            ast::Type::Int(_) => ScSpecTypeDef::I32,
            ast::Type::Uint(_) => ScSpecTypeDef::U32,
            ast::Type::Bool => ScSpecTypeDef::Bool,
            ast::Type::Address(_) => ScSpecTypeDef::Address,
            ast::Type::Bytes(_) | ast::Type::DynamicBytes => ScSpecTypeDef::Bytes,
            ast::Type::String => ScSpecTypeDef::String,
            ast::Type::Void => ScSpecTypeDef::Void,
            ast::Type::Struct(ast::StructType::UserDefined(n)) => Self::udt_spec_type(*n, ns),
            _ => panic!("unsupported type in spec: {ty:?}"),
        }
    }

    fn udt_spec_type(struct_no: usize, ns: &ast::Namespace) -> ScSpecTypeDef {
        let name = ns.structs[struct_no].id.name.as_str();
        ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: name
                .try_into()
                .unwrap_or_else(|_| panic!("struct name {name:?} exceeds the 60-char spec limit")),
        })
    }

    pub fn build<'a>(
        context: &'a Context,
        std_lib: &Module<'a>,
        contract: &'a ast::Contract,
        ns: &'a ast::Namespace,
        opt: &'a Options,
        contract_no: usize,
    ) -> Binary<'a> {
        let filename = ns.files[contract.loc.file_no()].file_name();
        let mut bin = Binary::new(
            context,
            ns,
            &contract.id.name,
            &filename,
            opt,
            std_lib,
            None,
        );

        let mut export_list = Vec::new();
        Self::declare_externals(&mut bin);
        Self::emit_functions_with_spec(contract, &mut bin, context, contract_no, &mut export_list);
        Self::emit_struct_spec_entries(context, contract, &mut bin);
        bin.internalize(export_list.as_slice());

        //Self::emit_initializer(&mut binary, ns, contract.constructors(ns).first());

        Self::emit_env_meta_entries(context, &mut bin, opt);

        bin
    }

    // In Soroban, the public functions specifications is embeded in the contract binary.
    // for each function, emit both the function spec entry and the function body.
    fn emit_functions_with_spec<'a>(
        contract: &'a ast::Contract,
        bin: &mut Binary<'a>,
        context: &'a Context,
        _contract_no: usize,
        export_list: &mut Vec<&'a str>,
    ) {
        let mut defines = Vec::new();

        for (cfg_no, cfg) in contract.cfg.iter().enumerate() {
            let ftype = bin.function_type(
                &cfg.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
                &cfg.returns.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
            );

            // For each function, determine the name and the linkage
            // Soroban has no dispatcher, so all externally addressable functions are exported and should be named the same as the original function name in the source code.
            // If there are duplicate function names, then the function name in the source is mangled to include the signature.

            // if func is a default constructor, then the function name is the contract name

            let linkage = if cfg.public {
                let name = if cfg.name.contains("::") {
                    // get the third part of the name which is the function name
                    cfg.name.split("::").collect::<Vec<&str>>()[2]
                } else {
                    &cfg.name
                };
                Self::emit_function_spec_entry(context, cfg, name.to_string(), bin);
                export_list.push(name);
                Linkage::External
            } else {
                Linkage::Internal
            };

            let func_decl = if let Some(func) = bin.module.get_function(&cfg.name) {
                // must not have a body yet
                assert_eq!(func.get_first_basic_block(), None);

                func
            } else {
                bin.module.add_function(&cfg.name, ftype, Some(linkage))
            };

            bin.functions.insert(cfg_no, func_decl);

            defines.push((func_decl, cfg));
        }

        let init_type = context.i64_type().fn_type(&[], false);
        bin.module
            .add_function("storage_initializer", init_type, None);

        for (func_decl, cfg) in defines {
            emit_cfg(&mut SorobanTarget, bin, contract, cfg, func_decl);
        }
    }

    fn emit_env_meta_entries<'a>(context: &'a Context, bin: &mut Binary<'a>, opt: &'a Options) {
        let mut meta = Limited::new(Vec::new(), Limits::none());
        let soroban_env_interface_version = opt.soroban_version;
        let soroban_env_interface_version = match soroban_env_interface_version {
            Some(version) => ScEnvMetaEntryInterfaceVersion {
                protocol: version.as_u32(),
                pre_release: 0,
            },
            None => SOROBAN_ENV_INTERFACE_VERSION,
        };
        ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(soroban_env_interface_version)
            .write_xdr(&mut meta)
            .expect("writing env meta interface version to xdr");
        Self::add_custom_section(context, &bin.module, "contractenvmetav0", meta.inner);
    }

    fn emit_function_spec_entry<'a>(
        context: &'a Context,
        cfg: &ControlFlowGraph,
        name: String,
        bin: &mut Binary<'a>,
    ) {
        if cfg.public && !cfg.is_placeholder() {
            let ns = bin.ns;
            let mut spec = Limited::new(Vec::new(), Limits::none());
            ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
                name: name
                    .try_into()
                    .unwrap_or_else(|_| panic!("function name {:?} exceeds limit", cfg.name)),
                inputs: cfg
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| ScSpecFunctionInputV0 {
                        name: p
                            .id
                            .as_ref()
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| i.to_string())
                            .try_into()
                            .expect("function input name exceeds limit"),
                        type_: Self::type_to_spec(&p.ty, ns),
                        doc: StringM::default(), // TODO: Add doc.
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("function input count exceeds limit"),
                outputs: cfg
                    .returns
                    .iter()
                    .map(|return_type| Self::type_to_spec(&return_type.ty, ns))
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("function output count exceeds limit"),
                doc: StringM::default(), // TODO: Add doc.
            })
            .write_xdr(&mut spec)
            .unwrap_or_else(|_| panic!("writing spec to xdr for function {}", cfg.name));

            Self::add_custom_section(context, &bin.module, "contractspecv0", spec.inner);
        }
    }

    fn emit_struct_spec_entries<'a>(
        context: &'a Context,
        contract: &'a ast::Contract,
        bin: &mut Binary<'a>,
    ) {
        let ns = bin.ns;
        let mut structs: Vec<usize> = Vec::new();
        for cfg in contract.cfg.iter() {
            if !cfg.public || cfg.is_placeholder() {
                continue;
            }
            for p in cfg.params.iter() {
                Self::collect_struct_deps(&p.ty, ns, &mut structs);
            }
            for p in cfg.returns.iter() {
                Self::collect_struct_deps(&p.ty, ns, &mut structs);
            }
        }

        for struct_no in structs {
            let decl = &ns.structs[struct_no];

            let name: StringM<60> = decl.id.name.as_str().try_into().unwrap_or_else(|_| {
                panic!(
                    "struct name {:?} exceeds the 60-char spec limit",
                    decl.id.name
                )
            });

            let fields = decl
                .fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let fname =
                        f.id.as_ref()
                            .map(|id| id.name.clone())
                            .unwrap_or_else(|| i.to_string());
                    ScSpecUdtStructFieldV0 {
                        doc: StringM::default(),
                        name: fname.as_str().try_into().unwrap_or_else(|_| {
                            panic!("struct field name {fname:?} exceeds the 30-char spec limit")
                        }),
                        type_: Self::type_to_spec(&f.ty, ns),
                    }
                })
                .collect::<Vec<_>>();

            let mut spec = Limited::new(Vec::new(), Limits::none());
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
                doc: StringM::default(),
                lib: StringM::default(),
                name,
                fields: fields
                    .try_into()
                    .expect("struct field count exceeds spec limit"),
            })
            .write_xdr(&mut spec)
            .unwrap_or_else(|_| panic!("writing struct spec entry for {:?}", decl.id.name));
            Self::add_custom_section(context, &bin.module, "contractspecv0", spec.inner);
        }
    }

    fn collect_struct_deps(ty: &ast::Type, ns: &ast::Namespace, acc: &mut Vec<usize>) {
        match ty {
            ast::Type::Ref(inner)
            | ast::Type::SorobanHandle(inner)
            | ast::Type::StorageRef(_, inner) => Self::collect_struct_deps(inner, ns, acc),
            ast::Type::Struct(ast::StructType::UserDefined(n)) => {
                if !acc.contains(n) {
                    acc.push(*n);
                    for f in &ns.structs[*n].fields {
                        Self::collect_struct_deps(&f.ty, ns, acc);
                    }
                }
            }
            ast::Type::Array(elem, _) => Self::collect_struct_deps(elem, ns, acc),
            _ => {}
        }
    }

    fn add_custom_section<'a>(
        context: &'a Context,
        module: &Module<'a>,
        name: &'a str,
        value: Vec<u8>,
    ) {
        let value_str = unsafe {
            // TODO: Figure out the right way to generate the LLVM metadata for
            // a slice of bytes.
            String::from_utf8_unchecked(value)
        };

        module
            .add_global_metadata(
                "wasm.custom_sections",
                &context.metadata_node(&[
                    context.metadata_string(name).into(),
                    context.metadata_string(&value_str).into(),
                ]),
            )
            .expect("adding spec as metadata");
    }

    fn declare_externals(bin: &mut Binary) {
        let host_functions = [
            HostFunctions::PutContractData,
            HostFunctions::GetContractData,
            HostFunctions::HasContractData,
            HostFunctions::DeleteContractData,
            HostFunctions::ExtendContractDataTtl,
            HostFunctions::ExtendCurrentContractInstanceAndCodeTtl,
            HostFunctions::LogFromLinearMemory,
            HostFunctions::SymbolNewFromLinearMemory,
            HostFunctions::VectorNew,
            HostFunctions::BytesNew,
            HostFunctions::Call,
            HostFunctions::VectorNewFromLinearMemory,
            HostFunctions::VecUnpackToLinearMemory,
            HostFunctions::ObjToU64,
            HostFunctions::ObjFromU64,
            HostFunctions::ObjToI64,
            HostFunctions::ObjFromI64,
            HostFunctions::PutContractData,
            HostFunctions::ObjToI128Lo64,
            HostFunctions::ObjToI128Hi64,
            HostFunctions::ObjToU128Lo64,
            HostFunctions::ObjToU128Hi64,
            HostFunctions::ObjFromI128Pieces,
            HostFunctions::ObjFromU128Pieces,
            HostFunctions::ObjToU256LoLo,
            HostFunctions::ObjToU256LoHi,
            HostFunctions::ObjToU256HiLo,
            HostFunctions::ObjToU256HiHi,
            HostFunctions::ObjFromU256Pieces,
            HostFunctions::ObjToI256LoLo,
            HostFunctions::ObjToI256LoHi,
            HostFunctions::ObjToI256HiLo,
            HostFunctions::ObjToI256HiHi,
            HostFunctions::ObjFromI256Pieces,
            HostFunctions::RequireAuth,
            HostFunctions::AuthAsCurrContract,
            HostFunctions::UpdateCurrentContractWasm,
            HostFunctions::MapNewFromLinearMemory,
            HostFunctions::MapNew,
            HostFunctions::MapPut,
            HostFunctions::MapGet,
            HostFunctions::MapDel,
            HostFunctions::MapHas,
            HostFunctions::VecPushBack,
            HostFunctions::VecGet,
            HostFunctions::VecPut,
            HostFunctions::StringNewFromLinearMemory,
            HostFunctions::StrKeyToAddr,
            HostFunctions::GetLedgerTimestamp,
            HostFunctions::GetLedgerSequence,
            HostFunctions::GetCurrentContractAddress,
            HostFunctions::BytesNewFromLinearMemory,
            HostFunctions::BytesCopyToLinearMemory,
            HostFunctions::BytesLen,
            HostFunctions::BytesGet,
            HostFunctions::BytesPut,
            HostFunctions::BytesPush,
            HostFunctions::BytesPop,
            HostFunctions::StringLen,
            HostFunctions::StringCopyToLinearMemory,
            HostFunctions::VecLen,
            HostFunctions::VecPopBack,
            HostFunctions::ContractEvent,
            HostFunctions::ComputeHashSha256,
            HostFunctions::ComputeHashKeccak256,
            HostFunctions::SerializeToBytes,
            HostFunctions::DeserializeFromBytes,
        ];

        for func in &host_functions {
            bin.module.add_function(
                func.name(),
                func.function_signature(bin),
                Some(Linkage::External),
            );
        }
    }
}
