// SPDX-License-Identifier: Apache-2.0

use clap::{Command, CommandFactory, FromArgMatches};

use clap_complete::generate;
use cli::PackageTrait;
use itertools::Itertools;
use solang::{
    abi,
    codegen::{codegen, Options},
    emit::Generate,
    file_resolver::FileResolver,
    sema::{ast::Namespace, file::PathDisplay},
    standard_json::{
        EwasmContract, JsonContract, JsonResult, OutputJson, StandardJsonInput, StandardJsonSource,
    },
};
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{self, create_dir, create_dir_all, File},
    io::{self, prelude::*},
    path::{Path, PathBuf},
    process::exit,
};

use crate::cli::{
    imports_arg, options_arg, target_arg, Cli, Commands, Compile, CompilerOutput, Doc, New,
    ShellComplete,
};

mod cli;
mod doc;
mod idl;
#[cfg(feature = "language_server")]
mod languageserver;

fn main() {
    let matches = Cli::command().get_matches();

    let cli = Cli::from_arg_matches(&matches).unwrap();

    match cli.command {
        Commands::Doc(doc_args) => doc(doc_args),
        Commands::Compile(compile_args) => {
            // Read config from configuration file. If extra args exist, only overwrite the fields that the user explicitly provides.
            let config = if let Some(conf_file) = &compile_args.configuration_file {
                if PathBuf::from(conf_file).exists() {
                    eprintln!("info: reading default config from toml file");
                    let debug = matches.subcommand_matches("compile").unwrap();
                    let mut compile = read_toml_config(conf_file);
                    compile.overwrite_with_matches(debug);

                    compile
                } else {
                    compile_args
                }
            } else {
                compile_args
            };
            compile(&config)
        }
        Commands::ShellComplete(shell_args) => shell_complete(Cli::command(), shell_args),
        #[cfg(feature = "language_server")]
        Commands::LanguageServer(server_args) => languageserver::start_server(&server_args),
        Commands::Idl(idl_args) => idl::idl(&idl_args),
        Commands::New(new_arg) => new_command(new_arg),
    }
}

fn read_toml_config(path: &OsString) -> Compile {
    let toml_data = fs::read_to_string(path).unwrap();

    let res: Result<Compile, _> = toml::from_str(&toml_data);

    match res {
        Ok(compile_args) => compile_args,
        Err(err) => {
            eprintln!("{err}");
            exit(1);
        }
    }
}

fn new_command(args: New) {
    let target = args.target_name.as_str();

    // Default project name is "solana_project" or "polkadot_project"
    let default_path = OsString::from(format!("{target}_project"));

    let dir_path = args.project_name.unwrap_or(default_path);

    if let Err(error) = create_dir(&dir_path) {
        eprintln!("couldn't create project directory, reason: {error}");
        exit(1);
    }

    let flipper = match target {
        "solana" => include_str!("../../examples/solana/flipper.sol"),
        "polkadot" => include_str!("../../examples/polkadot/flipper.sol"),
        "evm" => {
            eprintln!("EVM target is not supported yet!");
            exit(1);
        }
        _ => unreachable!(),
    };

    let mut flipper_file = create_file(&Path::new(&dir_path).join("flipper.sol"));
    flipper_file
        .write_all(flipper.to_string().as_bytes())
        .expect("failed to write flipper example");

    let mut toml_file = create_file(&Path::new(&dir_path).join("solang.toml"));

    let toml_content = match target {
        "solana" => include_str!("../../examples/solana/solana_config.toml"),
        "polkadot" => include_str!("../../examples/polkadot/polkadot_config.toml"),
        _ => unreachable!(),
    };
    toml_file
        .write_all(toml_content.to_string().as_bytes())
        .expect("failed to write example toml configuration file");
}

fn doc(doc_args: Doc) {
    let target = target_arg(&doc_args.target);
    let mut resolver: FileResolver = imports_arg(&doc_args.package);

    let verbose = doc_args.verbose;
    let mut success = true;
    let mut files = Vec::new();

    for filename in doc_args.package.input {
        let ns = solang::parse_and_resolve_with_options(
            filename.as_os_str(),
            &mut resolver,
            target,
            None,
        );

        ns.print_diagnostics(&resolver, verbose);

        if ns.contracts.is_empty() {
            eprintln!("{}: error: no contracts found", filename.to_string_lossy());
            success = false;
        } else if ns.diagnostics.any_errors() {
            success = false;
        } else {
            files.push(ns);
        }
    }

    if success {
        // generate docs
        doc::generate_docs(
            &doc_args
                .output_directory
                .unwrap_or_else(|| OsString::from(".")),
            &files,
            verbose,
        );
    }
}

fn compile(compile_args: &Compile) {
    let target = target_arg(&compile_args.target_arg);

    let mut json = JsonResult {
        errors: Vec::new(),
        target: target.to_string(),
        program: String::new(),
        contracts: HashMap::new(),
    };

    if compile_args.compiler_output.verbose {
        eprintln!("info: Solang version {}", env!("SOLANG_VERSION"));
    }

    let mut resolver = imports_arg(&compile_args.package);

    let compile_package = &compile_args.package;

    let opt = options_arg(
        &compile_args.debug_features,
        &compile_args.optimizations,
        compile_package,
    );

    let mut namespaces = Vec::new();

    let mut errors = false;
    let std_json = compile_args.compiler_output.std_json_output;

    // Build a map of requested contract names, and a flag specifying whether it was found or not
    let contract_names: HashSet<&str> = if let Some(values) = &compile_args.package.contracts {
        values.iter().map(String::as_str).collect()
    } else {
        HashSet::new()
    };

    let input_files = if std_json && compile_args.package.input.is_none() {
        load_standard_json_input(&mut resolver, &mut json)
    } else {
        compile_args.package.get_input().clone()
    };

    for filename in &input_files {
        // TODO: this could be parallelized using e.g. rayon
        let ns = process_file(
            filename,
            &mut resolver,
            target,
            &compile_args.compiler_output,
            &opt,
        );

        namespaces.push(ns);
    }

    let mut json_contracts = HashMap::new();

    for ns in &namespaces {
        if std_json {
            let mut out = ns.diagnostics_as_json(&resolver);
            json.errors.append(&mut out);
        } else {
            ns.print_diagnostics(&resolver, compile_args.compiler_output.verbose);
        }

        if ns.diagnostics.any_errors() {
            errors = true;
        }
    }
    if let Some("ast-dot") = compile_args.compiler_output.emit.as_deref() {
        exit(0);
    }

    // Ensure we have at least one contract
    if !errors && namespaces.iter().all(|ns| ns.contracts.is_empty()) {
        eprintln!("error: no contacts found");
        errors = true;
    }

    // Ensure we have all the requested contracts
    let not_found: Vec<_> = contract_names
        .iter()
        .filter(|name| {
            !namespaces
                .iter()
                .flat_map(|ns| ns.contracts.iter())
                .any(|contract| **name == contract.id.name)
        })
        .collect();

    if !errors && !not_found.is_empty() {
        eprintln!("error: contacts {} not found", not_found.iter().join(", "));
        errors = true;
    }

    if !errors {
        let mut seen_contracts = HashMap::new();

        let authors = if let Some(authors) = &compile_args.package.authors {
            if !target.is_polkadot() {
                eprintln!("warning: the `authors` flag will be ignored for {target} target")
            }
            authors.clone()
        } else {
            vec!["unknown".to_string()]
        };

        let version = if let Some(version) = &compile_args.package.version {
            version
        } else {
            "0.0.1"
        };

        for ns in &mut namespaces {
            for contract_no in 0..ns.contracts.len() {
                contract_results(
                    contract_no,
                    &compile_args.compiler_output,
                    ns,
                    &mut json_contracts,
                    &mut seen_contracts,
                    &opt,
                    &authors,
                    version,
                );
            }
        }
    }

    json.contracts = json_contracts;

    if std_json {
        println!("{}", serde_json::to_string(&json).unwrap());
        exit(0);
    }

    if errors {
        exit(1);
    }
}

fn shell_complete(mut app: Command, args: ShellComplete) {
    let name = app.get_name().to_string();
    generate(args.shell_complete, &mut app, name, &mut std::io::stdout());
}

fn output_file(compiler_output: &CompilerOutput, stem: &str, ext: &str, meta: bool) -> PathBuf {
    let dir = if meta {
        compiler_output
            .output_meta
            .as_ref()
            .or(compiler_output.output_directory.as_ref())
    } else {
        compiler_output.output_directory.as_ref()
    };
    Path::new(&dir.unwrap_or(&String::from("."))).join(format!("{stem}.{ext}"))
}

fn process_file(
    filename: &Path,
    resolver: &mut FileResolver,
    target: solang::Target,
    compiler_output: &CompilerOutput,
    opt: &Options,
) -> Namespace {
    let verbose = compiler_output.verbose;

    let filepath = match filename.canonicalize() {
        Ok(filename) => filename,
        Err(_) => filename.to_path_buf(),
    };

    // resolve phase
    let mut ns =
        solang::parse_and_resolve_with_options(filepath.as_os_str(), resolver, target, Some(opt));

    // codegen all the contracts; some additional errors/warnings will be detected here
    codegen(&mut ns, opt);

    if let Some("ast-dot") = compiler_output.emit.as_deref() {
        let stem = filepath.file_stem().unwrap().to_string_lossy();
        let dot_filename = output_file(compiler_output, &stem, "dot", false);

        if verbose {
            eprintln!("info: Saving graphviz dot {}", dot_filename.display());
        }

        let dot = ns.dotgraphviz();

        let mut file = create_file(&dot_filename);

        if let Err(err) = file.write_all(dot.as_bytes()) {
            eprintln!("{}: error: {}", dot_filename.display(), err);
            exit(1);
        }
    }

    ns
}

fn load_standard_json_input(resolver: &mut FileResolver, json: &mut JsonResult) -> Vec<PathBuf> {
    let mut stdin = String::new();

    if let Err(err) = io::stdin().read_to_string(&mut stdin) {
        standard_json_exit_with_error(json, format!("failed to read stdin: {err}"));
    }

    let input = match serde_json::from_str::<StandardJsonInput>(&stdin) {
        Ok(input) => input,
        Err(err) => standard_json_exit_with_error(
            json,
            format!("failed to parse standard json input: {err}"),
        ),
    };

    if let Some(language) = &input.language {
        if language != "Solidity" {
            standard_json_exit_with_error(
                json,
                format!("unsupported language '{language}', expected 'Solidity'"),
            );
        }
    }

    for remapping in &input.settings.remappings {
        let (prefix, path) = match remapping.split_once('=') {
            Some((prefix, path)) => (prefix, path),
            None => standard_json_exit_with_error(
                json,
                format!("invalid remapping '{remapping}', expected '<prefix>=<path>'"),
            ),
        };

        resolver.add_import_map(prefix.into(), PathBuf::from(path));
    }

    if input.sources.is_empty() {
        standard_json_exit_with_error(json, "standard json input did not contain any sources");
    }

    let mut input_files = Vec::with_capacity(input.sources.len());

    for (source_name, source) in input.sources {
        let contents = load_standard_json_source(&source_name, source, json);

        resolver.set_file_contents(&source_name, contents);
        input_files.push(PathBuf::from(source_name));
    }

    input_files
}

fn load_standard_json_source(
    source_name: &str,
    source: StandardJsonSource,
    json: &mut JsonResult,
) -> String {
    if let Some(content) = source.content {
        return content;
    }

    for url in source.urls {
        match fs::read_to_string(&url) {
            Ok(contents) => return contents,
            Err(_) => continue,
        }
    }

    standard_json_exit_with_error(
        json,
        format!(
            "source '{source_name}' is missing inline content and no readable urls were provided"
        ),
    );
}

fn standard_json_exit_with_error(json: &mut JsonResult, message: impl Into<String>) -> ! {
    let message = message.into();

    json.errors.push(OutputJson {
        sourceLocation: None,
        ty: "JSONError".to_owned(),
        component: "solang".to_owned(),
        severity: "error".to_owned(),
        formattedMessage: message.clone(),
        message,
    });

    println!("{}", serde_json::to_string(json).unwrap());
    exit(1);
}

fn contract_results(
    contract_no: usize,
    compiler_output: &CompilerOutput,
    ns: &mut Namespace,
    json_contracts: &mut HashMap<String, HashMap<String, JsonContract>>,
    seen_contracts: &mut HashMap<String, String>,
    opt: &Options,
    default_authors: &[String],
    version: &str,
) {
    let verbose = compiler_output.verbose;
    let std_json = compiler_output.std_json_output;

    let resolved_contract = &ns.contracts[contract_no];

    if !resolved_contract.instantiable {
        return;
    }

    if ns.top_file_no() != resolved_contract.loc.file_no() {
        // contracts that were imported should not be considered. For example, if we have a file
        // a.sol which imports b.sol, and b.sol defines contract B, then:
        // solang compile a.sol
        // should not write the results for contract B
        return;
    }

    let loc = ns.loc_to_string(PathDisplay::FullPath, &resolved_contract.loc);

    if let Some(other_loc) = seen_contracts.get(&resolved_contract.id.name) {
        eprintln!(
            "error: contract {} defined at {other_loc} and {}",
            resolved_contract.id, loc
        );
        exit(1);
    }

    seen_contracts.insert(resolved_contract.id.to_string(), loc);

    if let Some("cfg") = compiler_output.emit.as_deref() {
        println!("{}", resolved_contract.print_cfg(ns));
        return;
    }

    if verbose {
        if ns.target == solang::Target::Solana {
            eprintln!(
                "info: contract {} uses at least {} bytes account data",
                resolved_contract.id, resolved_contract.fixed_layout_size,
            );
        }

        eprintln!(
            "info: Generating LLVM IR for contract {} with target {}",
            resolved_contract.id, ns.target
        );
    }

    let context = inkwell::context::Context::create();

    let bin = resolved_contract.binary(ns, &context, opt, contract_no);

    if save_intermediates(&bin, compiler_output) {
        return;
    }

    let code = bin.code(Generate::Linked).expect("llvm build");

    #[cfg(feature = "wasm_opt")]
    if let Some(level) = opt.wasm_opt.filter(|_| ns.target.is_polkadot() && verbose) {
        eprintln!(
            "info: wasm-opt level '{}' for contract {}",
            level, resolved_contract.id
        );
    }

    if std_json {
        let source_name = ns.files[ns.top_file_no()]
            .path
            .to_string_lossy()
            .into_owned();

        json_contracts.entry(source_name).or_default().insert(
            bin.name,
            JsonContract {
                abi: abi::ethereum::gen_abi(contract_no, ns),
                ewasm: Some(EwasmContract {
                    wasm: hex::encode_upper(code),
                }),
                minimum_space: None,
            },
        );
    } else {
        let bin_filename = output_file(
            compiler_output,
            &bin.name,
            ns.target.file_extension(),
            false,
        );

        if verbose {
            eprintln!(
                "info: Saving binary {} for contract {}",
                bin_filename.display(),
                bin.name
            );
        }

        let mut file = create_file(&bin_filename);

        file.write_all(&code).unwrap();

        let (metadata, meta_ext) =
            abi::generate_abi(contract_no, ns, &code, verbose, default_authors, version);
        let meta_filename = output_file(compiler_output, &bin.name, meta_ext, true);

        if verbose {
            eprintln!(
                "info: Saving metadata {} for contract {}",
                meta_filename.display(),
                bin.name
            );
        }

        let mut file = create_file(&meta_filename);
        file.write_all(metadata.as_bytes()).unwrap();
    }
}

fn save_intermediates(
    bin: &solang::emit::binary::Binary,
    compiler_output: &CompilerOutput,
) -> bool {
    let verbose = compiler_output.verbose;

    match compiler_output.emit.as_deref() {
        Some("llvm-ir") => {
            let llvm_filename = output_file(compiler_output, &bin.name, "ll", false);

            if verbose {
                eprintln!(
                    "info: Saving LLVM IR {} for contract {}",
                    llvm_filename.display(),
                    bin.name
                );
            }

            bin.dump_llvm(&llvm_filename).unwrap();

            true
        }

        Some("llvm-bc") => {
            let bc_filename = output_file(compiler_output, &bin.name, "bc", false);

            if verbose {
                eprintln!(
                    "info: Saving LLVM BC {} for contract {}",
                    bc_filename.display(),
                    bin.name
                );
            }

            bin.bitcode(&bc_filename);

            true
        }

        Some("object") => {
            let obj = match bin.code(Generate::Object) {
                Ok(o) => o,
                Err(s) => {
                    println!("error: {s}");
                    exit(1);
                }
            };

            let obj_filename = output_file(compiler_output, &bin.name, "o", false);

            if verbose {
                eprintln!(
                    "info: Saving Object {} for contract {}",
                    obj_filename.display(),
                    bin.name
                );
            }

            let mut file = create_file(&obj_filename);
            file.write_all(&obj).unwrap();
            true
        }
        Some("asm") => {
            let obj = match bin.code(Generate::Assembly) {
                Ok(o) => o,
                Err(s) => {
                    println!("error: {s}");
                    exit(1);
                }
            };

            let obj_filename = output_file(compiler_output, &bin.name, "asm", false);

            if verbose {
                eprintln!(
                    "info: Saving Assembly {} for contract {}",
                    obj_filename.display(),
                    bin.name
                );
            }

            let mut file = create_file(&obj_filename);
            file.write_all(&obj).unwrap();
            true
        }
        Some("cfg") => true,
        Some("ast-dot") => true,
        _ => false,
    }
}

fn create_file(path: &Path) -> File {
    if let Some(parent) = path.parent() {
        if let Err(err) = create_dir_all(parent) {
            eprintln!(
                "error: cannot create output directory '{}': {}",
                parent.display(),
                err
            );
            exit(1);
        }
    }

    match File::create(path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("error: cannot create file '{}': {}", path.display(), err,);
            exit(1);
        }
    }
}
