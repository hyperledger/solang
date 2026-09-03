// SPDX-License-Identifier: Apache-2.0

use clap::{
    builder::ValueParser, parser::ValueSource, value_parser, ArgAction, ArgMatches, Args, Id,
    Parser, Subcommand,
};
use clap_complete::Shell;
#[cfg(feature = "wasm_opt")]
use contract_build::OptimizationPasses;

use itertools::Itertools;
use semver::Version;
use serde::Deserialize;
use solang::{
    codegen::{OptimizationLevel, Options},
    file_resolver::FileResolver,
    Target,
};
use std::{ffi::OsString, path::PathBuf, process::exit};

mod test;
#[derive(Parser)]
#[command(author = env!("CARGO_PKG_AUTHORS"), version = concat!("version ", env!("SOLANG_VERSION")), about = env!("CARGO_PKG_DESCRIPTION"), subcommand_required = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Compile Solidity source files")]
    Compile(Compile),

    #[command(about = "Generate documention for contracts using doc comments")]
    Doc(Doc),

    #[command(about = "Print shell completion for various shells to STDOUT")]
    ShellComplete(ShellComplete),

    #[cfg(feature = "language_server")]
    #[command(about = "Start LSP language server on stdin/stdout")]
    LanguageServer(LanguageServerCommand),

    #[command(about = "Generate Solidity interface files from Anchor IDL files")]
    Idl(IdlCommand),

    #[command(about = "Create a new Solang project")]
    New(New),
}

#[derive(Args)]
pub struct New {
    #[arg(name = "TARGETNAME",required= true, long = "target", value_parser = ["solana", "polkadot", "evm"], help = "Target to build for [possible values: solana, polkadot]", num_args = 1, hide_possible_values = true)]
    pub target_name: String,

    #[arg(name = "INPUT", help = "Name of the project", num_args = 1, value_parser = ValueParser::os_string())]
    pub project_name: Option<OsString>,
}

#[derive(Args)]
pub struct IdlCommand {
    #[arg(name = "INPUT", help = "Convert IDL files", required= true, value_parser = ValueParser::os_string(), num_args = 1..)]
    pub input: Vec<OsString>,

    #[arg(name = "OUTPUT",help = "output file", short = 'o', long = "output", num_args = 1, value_parser =ValueParser::path_buf())]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct LanguageServerCommand {
    #[clap(flatten)]
    pub target: TargetArg,

    #[arg(name = "IMPORTPATH", help = "Directory to search for solidity files", value_parser = ValueParser::path_buf(), action = ArgAction::Append, long = "importpath", short = 'I', num_args = 1)]
    pub import_path: Option<Vec<PathBuf>>,

    #[arg(name = "IMPORTMAP", help = "Map directory to search for solidity files [format: map=path]",value_parser = ValueParser::new(parse_import_map), action = ArgAction::Append, long = "importmap", short = 'm', num_args = 1)]
    pub import_map: Option<Vec<(String, PathBuf)>>,
}

#[derive(Args)]
pub struct ShellComplete {
    #[arg(required = true, value_parser = value_parser!(Shell), help = "Name of a supported shell")]
    pub shell_complete: Shell,
}

#[derive(Args)]
pub struct Doc {
    #[clap(flatten)]
    pub package: DocPackage,

    #[clap(flatten)]
    pub target: TargetArg,

    #[arg(name = "VERBOSE" ,help = "show debug messages", short = 'v', action = ArgAction::SetTrue, long = "verbose")]
    pub verbose: bool,

    #[arg(name = "OUTPUT",help = "output directory", short = 'o', long = "output", num_args = 1, value_parser =ValueParser::string())]
    pub output_directory: Option<OsString>,
}

#[derive(Args, Deserialize, Debug, PartialEq)]
pub struct Compile {
    #[arg(name = "CONFFILE", help = "Take arguments from configuration file", long = "config-file", value_parser = ValueParser::os_string(), num_args = 0..=1, default_value = "solang.toml")]
    #[serde(skip)]
    pub configuration_file: Option<OsString>,

    #[clap(flatten)]
    pub package: CompilePackage,

    #[clap(flatten)]
    #[serde(
        default = "CompilerOutput::default",
        rename(deserialize = "compiler-output")
    )]
    pub compiler_output: CompilerOutput,

    #[clap(flatten)]
    #[serde(rename(deserialize = "target"))]
    pub target_arg: CompileTargetArg,

    #[clap(flatten)]
    #[serde(default = "DebugFeatures::default")]
    pub debug_features: DebugFeatures,

    #[clap(flatten)]
    #[serde(default = "Optimizations::default")]
    pub optimizations: Optimizations,
}

impl Compile {
    /// loop over args explicitly provided at runtime and update Compile accordingly.
    pub fn overwrite_with_matches(&mut self, matches: &ArgMatches) -> &mut Compile {
        for id in explicit_args(matches) {
            match id.as_str() {
                // Package args
                "INPUT" => {
                    self.package.input = matches
                        .get_many::<PathBuf>("INPUT")
                        .map(|input_paths| input_paths.map(PathBuf::from).collect())
                }
                "CONTRACT" => {
                    self.package.contracts = matches
                        .get_many::<String>("CONTRACT")
                        .map(|contract_names| contract_names.map(String::from).collect())
                }
                "IMPORTPATH" => {
                    self.package.import_path = matches
                        .get_many::<PathBuf>("IMPORTPATH")
                        .map(|paths| paths.map(PathBuf::from).collect())
                }
                "IMPORTMAP" => {
                    self.package.import_map = matches
                        .get_many::<(String, PathBuf)>("IMPORTMAP")
                        .map(|import_map| import_map.cloned().collect())
                }
                "AUTHOR" => {
                    self.package.authors = matches
                        .get_many::<String>("AUTHOR")
                        .map(|contract_names| contract_names.map(String::from).collect())
                }
                "VERSION" => self.package.version = matches.get_one::<String>("VERSION").cloned(),

                // CompilerOutput args
                "EMIT" => self.compiler_output.emit = matches.get_one::<String>("EMIT").cloned(),
                "OUTPUT" => {
                    self.compiler_output.output_directory =
                        matches.get_one::<String>("OUTPUT").cloned()
                }
                "OUTPUTMETA" => {
                    self.compiler_output.output_meta =
                        matches.get_one::<String>("OUTPUTMETA").cloned()
                }
                "STD-JSON" => {
                    self.compiler_output.std_json_output =
                        *matches.get_one::<bool>("STD-JSON").unwrap()
                }
                "VERBOSE" => {
                    self.compiler_output.verbose = *matches.get_one::<bool>("VERBOSE").unwrap()
                }

                // DebugFeatures args
                "NOLOGRUNTIMEERRORS" => {
                    self.debug_features.log_runtime_errors =
                        *matches.get_one::<bool>("NOLOGRUNTIMEERRORS").unwrap()
                }
                "NOPRINTS" => {
                    self.debug_features.log_prints = *matches.get_one::<bool>("NOPRINTS").unwrap()
                }
                "GENERATEDEBUGINFORMATION" => {
                    self.debug_features.generate_debug_info =
                        *matches.get_one::<bool>("GENERATEDEBUGINFORMATION").unwrap()
                }
                "RELEASE" => {
                    self.debug_features.release = *matches.get_one::<bool>("RELEASE").unwrap()
                }

                // Optimizations args
                "DEADSTORAGE" => {
                    self.optimizations.dead_storage =
                        *matches.get_one::<bool>("DEADSTORAGE").unwrap()
                }
                "CONSTANTFOLDING" => {
                    self.optimizations.constant_folding =
                        *matches.get_one::<bool>("CONSTANTFOLDING").unwrap()
                }
                "STRENGTHREDUCE" => {
                    self.optimizations.strength_reduce =
                        *matches.get_one::<bool>("STRENGTHREDUCE").unwrap()
                }
                "VECTORTOSLICE" => {
                    self.optimizations.vector_to_slice =
                        *matches.get_one::<bool>("VECTORTOSLICE").unwrap()
                }
                "COMMONSUBEXPRESSIONELIMINATION" => {
                    self.optimizations.common_subexpression_elimination = *matches
                        .get_one::<bool>("COMMONSUBEXPRESSIONELIMINATION")
                        .unwrap()
                }
                "OPT" => self.optimizations.opt_level = matches.get_one::<String>("OPT").cloned(),

                "TARGET" => self.target_arg.name = matches.get_one::<String>("TARGET").cloned(),
                "ADDRESS_LENGTH" => {
                    self.target_arg.address_length =
                        matches.get_one::<u64>("ADDRESS_LENGTH").copied()
                }
                "VALUE_LENGTH" => {
                    self.target_arg.value_length = matches.get_one::<u64>("VALUE_LENGTH").copied()
                }

                _ => {}
            }
        }

        self
    }
}

#[derive(Args, Deserialize, Default, Debug, PartialEq)]
pub struct CompilerOutput {
    #[arg(name = "EMIT", help = "Emit compiler state at early stage", long = "emit", num_args = 1, value_parser = ["ast-dot", "cfg", "llvm-ir", "llvm-bc", "object", "asm"])]
    #[serde(deserialize_with = "deserialize_emit", default)]
    pub emit: Option<String>,

    #[arg(name = "STD-JSON",help = "mimic solidity json output on stdout", conflicts_with_all = ["VERBOSE", "OUTPUT", "EMIT"], action = ArgAction::SetTrue, long = "standard-json")]
    #[serde(default)]
    pub std_json_output: bool,

    #[arg(name = "OUTPUT",help = "output directory", short = 'o', long = "output", num_args = 1, value_parser =ValueParser::string())]
    #[serde(default)]
    pub output_directory: Option<String>,

    #[arg(name = "OUTPUTMETA",help = "output directory for metadata", long = "output-meta", num_args = 1, value_parser = ValueParser::string())]
    #[serde(default)]
    pub output_meta: Option<String>,

    #[arg(name = "VERBOSE" ,help = "show debug messages", short = 'v', action = ArgAction::SetTrue, long = "verbose")]
    #[serde(default)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct TargetArg {
    #[arg(name = "TARGET",required= true, long = "target", value_parser = ["solana", "polkadot", "evm"], help = "Target to build for [possible values: solana, polkadot]", num_args = 1, hide_possible_values = true)]
    pub name: String,

    #[arg(name = "ADDRESS_LENGTH", help = "Address length on the Polkadot Parachain", long = "address-length", num_args = 1, value_parser = value_parser!(u64).range(4..1024))]
    pub address_length: Option<u64>,

    #[arg(name = "VALUE_LENGTH", help = "Value length on the Polkadot Parachain", long = "value-length", num_args = 1, value_parser = value_parser!(u64).range(4..1024))]
    pub value_length: Option<u64>,
}

#[derive(Args, Deserialize, Debug, PartialEq)]
pub struct CompileTargetArg {
    #[arg(name = "TARGET", long = "target", value_parser = ["solana", "polkadot", "evm", "soroban"], help = "Target to build for [possible values: solana, polkadot]", num_args = 1, hide_possible_values = true)]
    pub name: Option<String>,

    #[arg(name = "ADDRESS_LENGTH", help = "Address length on the Polkadot Parachain", long = "address-length", num_args = 1, value_parser = value_parser!(u64).range(4..1024))]
    pub address_length: Option<u64>,

    #[arg(name = "VALUE_LENGTH", help = "Value length on the Polkadot Parachain", long = "value-length", num_args = 1, value_parser = value_parser!(u64).range(4..1024))]
    pub value_length: Option<u64>,
}

#[derive(Args)]
pub struct DocPackage {
    #[arg(name = "INPUT", help = "Solidity input files",value_parser = ValueParser::path_buf(), num_args = 1.., required = true)]
    pub input: Vec<PathBuf>,

    #[arg(name = "CONTRACT", help = "Contract names to compile (defaults to all)", value_delimiter = ',', action = ArgAction::Append, long = "contract")]
    pub contracts: Option<Vec<String>>,

    #[arg(name = "IMPORTPATH", help = "Directory to search for solidity files",value_parser = ValueParser::path_buf(), action = ArgAction::Append, long = "importpath", short = 'I', num_args = 1)]
    pub import_path: Option<Vec<PathBuf>>,

    #[arg(name = "IMPORTMAP", help = "Map directory to search for solidity files [format: map=path]",value_parser = ValueParser::new(parse_import_map), action = ArgAction::Append, long = "importmap", short = 'm', num_args = 1)]
    pub import_map: Option<Vec<(String, PathBuf)>>,
}

#[derive(Args, Deserialize, Debug, PartialEq)]
pub struct CompilePackage {
    #[arg(name = "INPUT", help = "Solidity input files",value_parser = ValueParser::path_buf(), num_args = 1..)]
    #[serde(rename(deserialize = "input_files"))]
    pub input: Option<Vec<PathBuf>>,

    #[arg(name = "CONTRACT", help = "Contract names to compile (defaults to all)", value_delimiter = ',', action = ArgAction::Append, long = "contract")]
    pub contracts: Option<Vec<String>>,

    #[arg(name = "IMPORTPATH", help = "Directory to search for solidity files", value_parser = ValueParser::path_buf(), action = ArgAction::Append, long = "importpath", short = 'I', num_args = 1)]
    pub import_path: Option<Vec<PathBuf>>,

    #[arg(name = "IMPORTMAP", help = "Map directory to search for solidity files [format: map=path]",value_parser = ValueParser::new(parse_import_map), action = ArgAction::Append, long = "importmap", short = 'm', num_args = 1)]
    #[serde(deserialize_with = "deserialize_inline_table", default)]
    pub import_map: Option<Vec<(String, PathBuf)>>,

    #[arg(name = "AUTHOR", help = "specify contracts authors", long = "contract-authors", value_delimiter = ',', action = ArgAction::Append)]
    #[serde(default)]
    pub authors: Option<Vec<String>>,

    #[arg(name = "VERSION", help = "specify contracts version", long = "version", num_args = 1, value_parser = ValueParser::new(parse_version))]
    #[serde(default, deserialize_with = "deserialize_version")]
    pub version: Option<String>,

    #[arg(
        name = "SOROBAN-VERSION",
        help = "specify soroban contracts pre-release number",
        short = 's',
        long = "soroban-version",
        num_args = 1
    )]
    pub soroban_version: Option<u64>,
}

#[derive(Args, Deserialize, Debug, PartialEq)]
pub struct DebugFeatures {
    #[arg(name = "NOLOGRUNTIMEERRORS", help = "Disable logging runtime errors in the environment", long = "no-log-runtime-errors", action = ArgAction::SetFalse)]
    #[serde(default, rename(deserialize = "log-runtime-errors"))]
    pub log_runtime_errors: bool,

    #[arg(name = "NOPRINTS", help = "Disable logging prints in the environment", long = "no-prints", action = ArgAction::SetFalse)]
    #[serde(default = "default_true", rename(deserialize = "prints"))]
    pub log_prints: bool,

    #[arg(name = "GENERATEDEBUGINFORMATION", help = "Enable generating debug information for LLVM IR", long = "generate-debug-info", action = ArgAction::SetTrue, short = 'g')]
    #[serde(default, rename(deserialize = "generate-debug-info"))]
    pub generate_debug_info: bool,

    #[arg(name = "RELEASE", help = "Disable all debugging features such as prints, logging runtime errors, and logging api return codes", long = "release", action = ArgAction::SetTrue)]
    #[serde(default)]
    pub release: bool,

    #[arg(name = "STRICTSOROBANTYPES", help = "Turn Soroban integer width warnings into errors for stricter type safety", long = "strict-soroban-types", action = ArgAction::SetTrue)]
    #[serde(default)]
    pub strict_soroban_types: bool,
}

impl Default for DebugFeatures {
    fn default() -> Self {
        DebugFeatures {
            log_runtime_errors: true,
            log_prints: true,
            generate_debug_info: false,
            release: false,
            strict_soroban_types: false,
        }
    }
}

#[derive(Args, Deserialize, Default, Debug, PartialEq)]
pub struct Optimizations {
    #[arg(name = "DEADSTORAGE", help = "Disable dead storage codegen optimization", long = "no-dead-storage", action = ArgAction::SetFalse, display_order = 3)]
    #[serde(default = "default_true", rename(deserialize = "dead-storage"))]
    pub dead_storage: bool,

    #[arg(name = "CONSTANTFOLDING", help = "Disable constant folding codegen optimization", long = "no-constant-folding", action = ArgAction::SetFalse, display_order = 1)]
    #[serde(default = "default_true", rename(deserialize = "constant-folding"))]
    pub constant_folding: bool,

    #[arg(name = "STRENGTHREDUCE", help = "Disable strength reduce codegen optimization", long = "no-strength-reduce", action = ArgAction::SetFalse, display_order = 2)]
    #[serde(default = "default_true", rename(deserialize = "strength-reduce"))]
    pub strength_reduce: bool,

    #[arg(name = "VECTORTOSLICE", help = "Disable vector to slice codegen optimization", long = "no-vector-to-slice", action = ArgAction::SetFalse, display_order = 4)]
    #[serde(default = "default_true", rename(deserialize = "vector-to-slice"))]
    pub vector_to_slice: bool,

    #[arg(name = "COMMONSUBEXPRESSIONELIMINATION", help = "Disable common subexpression elimination", long = "no-cse", action = ArgAction::SetFalse, display_order = 5)]
    #[serde(
        default = "default_true",
        rename(deserialize = "common-subexpression-elimination")
    )]
    pub common_subexpression_elimination: bool,

    #[arg(name = "OPT", help = "Set LLVM optimizer level (0, 1, 2, 3, s, z)", short = 'O', default_value = "2", value_parser = ["0", "1", "2", "3", "s", "z", "none", "less", "default", "aggressive"], num_args = 1)]
    #[serde(rename(deserialize = "llvm-IR-optimization-level"))]
    pub opt_level: Option<String>,

    #[cfg(feature = "wasm_opt")]
    #[arg(
        name = "WASM_OPT",
        help = "wasm-opt passes for Wasm targets (0, 1, 2, 3, 4, s or z; see the wasm-opt help for more details)",
        long = "wasm-opt",
        num_args = 1
    )]
    #[serde(rename(deserialize = "wasm-opt"))]
    pub wasm_opt_passes: Option<OptimizationPasses>,
}

pub trait TargetArgTrait {
    fn get_name(&self) -> &String;
    fn get_address_length(&self) -> &Option<u64>;
    fn get_value_length(&self) -> &Option<u64>;
}

impl TargetArgTrait for TargetArg {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_address_length(&self) -> &Option<u64> {
        &self.address_length
    }

    fn get_value_length(&self) -> &Option<u64> {
        &self.value_length
    }
}

impl TargetArgTrait for CompileTargetArg {
    fn get_name(&self) -> &String {
        if let Some(name) = &self.name {
            name
        } else {
            eprintln!("error: no target name specified");
            exit(1);
        }
    }

    fn get_address_length(&self) -> &Option<u64> {
        &self.address_length
    }

    fn get_value_length(&self) -> &Option<u64> {
        &self.value_length
    }
}

pub(crate) fn target_arg<T: TargetArgTrait>(target_arg: &T) -> Target {
    let target_name = target_arg.get_name();

    if target_name == "solana" || target_name == "evm" {
        if target_arg.get_address_length().is_some() {
            eprintln!("error: address length cannot be modified except for polkadot target");
            exit(1);
        }

        if target_arg.get_value_length().is_some() {
            eprintln!("error: value length cannot be modified except for polkadot target");
            exit(1);
        }
    }

    let target = match target_name.as_str() {
        "solana" => solang::Target::Solana,
        "polkadot" => solang::Target::Polkadot {
            address_length: target_arg.get_address_length().unwrap_or(32) as usize,
            value_length: target_arg.get_value_length().unwrap_or(16) as usize,
        },
        "evm" => solang::Target::EVM,
        "soroban" => solang::Target::Soroban,
        _ => unreachable!(),
    };

    target
}

/// This trait is used to avoid code repetition when dealing with two implementations of the Package type:
/// `CompilePackage` and `DocPackage`. Each struct represents a group of arguments for the compile and doc commands.
/// Throughout the code, these two structs are treated the same, and this trait allows for unified handling.
pub trait PackageTrait {
    fn get_input(&self) -> &Vec<PathBuf>;
    fn get_import_path(&self) -> &Option<Vec<PathBuf>>;
    fn get_import_map(&self) -> &Option<Vec<(String, PathBuf)>>;
}

impl PackageTrait for CompilePackage {
    fn get_input(&self) -> &Vec<PathBuf> {
        if let Some(files) = &self.input {
            files
        } else {
            eprintln!(
                "No input files specified, please specifiy them in solang.toml or in command line"
            );
            exit(1);
        }
    }

    fn get_import_path(&self) -> &Option<Vec<PathBuf>> {
        &self.import_path
    }

    fn get_import_map(&self) -> &Option<Vec<(String, PathBuf)>> {
        &self.import_map
    }
}

impl PackageTrait for DocPackage {
    fn get_input(&self) -> &Vec<PathBuf> {
        &self.input
    }

    fn get_import_path(&self) -> &Option<Vec<PathBuf>> {
        &self.import_path
    }

    fn get_import_map(&self) -> &Option<Vec<(String, PathBuf)>> {
        &self.import_map
    }
}

pub fn imports_arg<T: PackageTrait>(package: &T) -> FileResolver {
    let mut resolver = FileResolver::default();

    if let Some(paths) = package.get_import_path() {
        let dups: Vec<_> = paths.iter().duplicates().collect();

        if !dups.is_empty() {
            eprintln!(
                "error: import paths {} specified more than once",
                dups.iter().map(|p| format!("'{}'", p.display())).join(", ")
            );
            exit(1);
        }

        for path in paths {
            resolver.add_import_path(path);
        }
    }

    if let Some(maps) = package.get_import_map() {
        for (map, path) in maps {
            let os_map = OsString::from(map);
            if let Some((_, existing_path)) = resolver
                .get_import_paths()
                .iter()
                .find(|(m, _)| *m == Some(os_map.clone()))
            {
                eprintln!(
                    "warning: mapping '{}' to '{}' is overwritten",
                    map,
                    existing_path.display()
                )
            }
            resolver.add_import_map(os_map, path.clone());
        }
    }

    resolver
}

pub fn options_arg(
    debug: &DebugFeatures,
    optimizations: &Optimizations,
    compiler_inputs: &CompilePackage,
) -> Options {
    let (opt_level, opt_size_level) = if let Some(level) = &optimizations.opt_level {
        match level.as_str() {
            // Standard GCC/Clang-style optimization levels
            "0" | "none" => (OptimizationLevel::None, 0),
            "1" | "less" => (OptimizationLevel::Less, 0),
            "2" | "default" => (OptimizationLevel::Default, 0),
            "3" | "aggressive" => (OptimizationLevel::Aggressive, 0),
            "s" => (OptimizationLevel::Default, 1),
            "z" => (OptimizationLevel::Default, 2),
            _ => unreachable!(),
        }
    } else {
        (OptimizationLevel::Default, 0)
    };

    Options {
        dead_storage: optimizations.dead_storage,
        constant_folding: optimizations.constant_folding,
        strength_reduce: optimizations.strength_reduce,
        vector_to_slice: optimizations.vector_to_slice,
        common_subexpression_elimination: optimizations.common_subexpression_elimination,
        generate_debug_information: debug.generate_debug_info,
        opt_level,
        opt_size_level,
        log_runtime_errors: debug.log_runtime_errors && !debug.release,
        log_prints: debug.log_prints && !debug.release,
        strict_soroban_types: debug.strict_soroban_types,
        #[cfg(feature = "wasm_opt")]
        wasm_opt: optimizations.wasm_opt_passes.or(if debug.release {
            Some(OptimizationPasses::Z)
        } else {
            None
        }),
        soroban_version: compiler_inputs.soroban_version,
    }
}

// Parse the import map argument. This takes the form
/// --import-map openzeppelin=/opt/openzeppelin-contracts/contract,
/// and returns the name of the map and the path.
fn parse_import_map(map: &str) -> Result<(String, PathBuf), String> {
    if let Some((var, value)) = map.split_once('=') {
        Ok((var.to_owned(), PathBuf::from(value)))
    } else {
        Err("contains no '='".to_owned())
    }
}

fn parse_version(version: &str) -> Result<String, String> {
    match Version::parse(version) {
        Ok(version) => Ok(version.to_string()),
        Err(err) => Err(err.to_string()),
    }
}

fn deserialize_inline_table<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<(String, PathBuf)>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let res: Option<toml::Table> = Option::deserialize(deserializer)?;

    match res {
        Some(table) => Ok(Some(
            table
                .iter()
                .map(|f| {
                    (
                        f.0.clone(),
                        if f.1.is_str() {
                            PathBuf::from(f.1.as_str().unwrap())
                        } else {
                            let key = f.1;
                            eprintln!("error: invalid value for import map {key}");
                            exit(1)
                        },
                    )
                })
                .collect(),
        )),
        None => Ok(None),
    }
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let res: Option<String> = Option::deserialize(deserializer)?;

    match res {
        Some(version) => match Version::parse(&version) {
            Ok(version) => Ok(Some(version.to_string())),
            Err(err) => Err(serde::de::Error::custom(err)),
        },
        None => Ok(None),
    }
}

fn deserialize_emit<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let str: Option<String> = Option::deserialize(deserializer)?;
    match str {
        Some(value) => {
            match value.as_str() {
                "ast-dot"|"cfg"|"llvm-ir"|"llvm-bc"|"object"|"asm" =>
                    Ok(Some(value))
                ,
                _ => Err(serde::de::Error::custom("Invalid option for `emit`. Valid options are: `ast-dot`, `cfg`, `llvm-ir`, `llvm-bc`, `object`, `asm`"))
            }
        }
        None => Ok(None),
    }
}

fn default_true() -> bool {
    true
}

/// Get args provided explicitly at runtime.
fn explicit_args(matches: &ArgMatches) -> Vec<&Id> {
    matches
        .ids()
        .filter(|x| {
            matches!(
                matches.value_source(x.as_str()),
                Some(ValueSource::CommandLine)
            )
        })
        .collect()
}
