// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

mod tests {
    use crate::{cli, options_arg, Cli, Commands};
    use clap::{CommandFactory, Parser};
    use solang::codegen::Options;
    use std::path::PathBuf;

    #[test]
    fn test() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_compile_options() {
        let mut command: Vec<&str> = "solang compile flipper.sol --target polkadot --value-length=31 --address-length=33 --no-dead-storage --no-constant-folding --no-strength-reduce --no-vector-to-slice --no-cse -O 3".split(' ').collect();
        let mut cli = Cli::parse_from(command);

        if let Commands::Compile(compile_args) = cli.command {
            assert_eq!(
                compile_args.package.input.unwrap(),
                vec![PathBuf::from("flipper.sol")]
            );
            assert_eq!(compile_args.target_arg.name.unwrap(), "polkadot");
            assert_eq!(compile_args.target_arg.address_length.unwrap(), 33_u64);
            assert_eq!(compile_args.target_arg.value_length.unwrap(), 31_u64);
            assert!(!compile_args.optimizations.common_subexpression_elimination,);
            assert!(!compile_args.optimizations.constant_folding);
            assert!(!compile_args.optimizations.dead_storage);
            assert!(!compile_args.optimizations.vector_to_slice);
            assert!(!compile_args.optimizations.strength_reduce);
            assert_eq!(compile_args.optimizations.opt_level.unwrap(), "3");
        }

        command = "solang compile flipper.sol --target polkadot --no-log-runtime-errors --no-prints -g --release".split(' ').collect();
        cli = Cli::parse_from(command);

        if let Commands::Compile(compile_args) = cli.command {
            assert!(compile_args.debug_features.generate_debug_info);
            assert!(!compile_args.debug_features.log_prints);
            assert!(!compile_args.debug_features.log_runtime_errors);
            assert!(compile_args.debug_features.release);
        }
    }

    #[test]
    fn parse_package_from_toml() {
        let mut package_toml = r#"
        input_files = ["flipper.sol"]   # Files to be compiled. You can define multiple files as : input_files = ["file1", "file2", ..]
        contracts = ["flipper"] # Contracts to include from the compiled files
        import_path = ["path1", "path2"]
        import_map = {map1="path", map2="path2"}    # Maps to import. Define as : import_paths = ["map=path/to/map", "map2=path/to/map2", ..]"#;

        let package: cli::CompilePackage = toml::from_str(package_toml).unwrap();

        assert_eq!(package.input.unwrap(), [PathBuf::from("flipper.sol")]);
        assert_eq!(package.contracts.unwrap(), ["flipper".to_owned()]);
        assert_eq!(
            package.import_path.unwrap(),
            [PathBuf::from("path1"), PathBuf::from("path2")]
        );
        assert_eq!(
            package.import_map.unwrap(),
            [
                ("map1".to_owned(), PathBuf::from("path")),
                ("map2".to_owned(), PathBuf::from("path2"))
            ]
        );

        package_toml = r#"
            input_files = ["flipper.sol"]
            import_map = ["map_name.path"]
        "#;

        let res: Result<cli::CompilePackage, _> = toml::from_str(package_toml);

        match res {
            Ok(_) => unreachable!(),
            Err(error) => {
                assert_eq!("invalid type: sequence, expected a map", error.message())
            }
        }
    }

    #[test]
    fn parse_options_toml() {
        let toml_debug_features = r#"
        prints = true
    log-runtime-errors = false
    generate-debug-info = false"#;

        let debug_features: cli::DebugFeatures = toml::from_str(toml_debug_features).unwrap();

        assert!(debug_features.log_prints);
        assert!(!debug_features.log_runtime_errors);
        assert!(!debug_features.generate_debug_info);

        let default_debug: cli::DebugFeatures = toml::from_str("").unwrap();

        let default_optimize: cli::Optimizations = toml::from_str("").unwrap();

        let compiler_package = cli::CompilePackage {
            input: Some(vec![PathBuf::from("flipper.sol")]),
            contracts: Some(vec!["flipper".to_owned()]),
            import_path: Some(vec![]),
            import_map: Some(vec![]),
            authors: None,
            version: Some("0.1.0".to_string()),
            soroban_version: None,
        };

        let opt = options_arg(&default_debug, &default_optimize, &compiler_package);

        assert_eq!(opt, Options::default());

        let opt_toml = r#"
        dead-storage = false
        constant-folding = false
        strength-reduce = false
        vector-to-slice = false
        common-subexpression-elimination = true
        llvm-IR-optimization-level = "3""#;

        let opt: cli::Optimizations = toml::from_str(opt_toml).unwrap();

        assert!(opt.common_subexpression_elimination);
        assert!(!opt.dead_storage);
        assert!(!opt.constant_folding);
        assert!(!opt.strength_reduce);
        assert!(!opt.vector_to_slice);
        assert_eq!(opt.opt_level.unwrap(), "3");
    }

    #[cfg(feature = "wasm_opt")]
    #[test]
    fn wasm_opt_option() {
        use contract_build::OptimizationPasses;

        let opt: cli::Optimizations = toml::from_str(r#"wasm-opt = "Zero""#).unwrap();
        assert_eq!(opt.wasm_opt_passes, Some(OptimizationPasses::Zero));
    }

    #[test]
    fn parse_target() {
        let target_toml = r#"
        name = "polkadot"  # Valid targets are "solana" and "polkadot"
        address_length = 32
        value_length = 16"#;

        let target: cli::CompileTargetArg = toml::from_str(target_toml).unwrap();

        assert_eq!(target.name.unwrap(), "polkadot");
        assert_eq!(target.address_length.unwrap(), 32);
        assert_eq!(target.value_length.unwrap(), 16);
    }

    #[test]
    fn parse_compiler_output() {
        let compiler_out = r#"
        verbose = true
        std_json_output = false
        emit = "ast-dot"
        output_directory = "output"
        output_meta = "metadata"
        "#;

        let out: cli::CompilerOutput = toml::from_str(compiler_out).unwrap();

        assert!(out.verbose);
        assert!(!out.std_json_output);
        assert_eq!(out.emit, Some("ast-dot".to_owned()));
        assert_eq!(out.output_directory, Some("output".to_owned()));
        assert_eq!(out.output_meta, Some("metadata".to_owned()));

        let default_out: cli::CompilerOutput = toml::from_str("").unwrap();

        assert!(!default_out.verbose);
        assert!(!default_out.std_json_output);
    }

    #[test]
    fn overwrite_with_matches() {
        let toml = include_str!("../../../examples/solana/solana_config.toml");
        let mut compile_config: cli::Compile = toml::from_str(toml).unwrap();

        assert_eq!(
            compile_config,
            cli::Compile {
                configuration_file: None,
                package: cli::CompilePackage {
                    input: Some(vec![PathBuf::from("flipper.sol")]),
                    contracts: Some(vec!["flipper".to_owned()]),
                    import_path: Some(vec![]),
                    import_map: Some(vec![]),
                    authors: None,
                    version: Some("0.1.0".to_string()),
                    soroban_version: None
                },
                compiler_output: cli::CompilerOutput {
                    emit: None,
                    std_json_output: false,
                    output_directory: None,
                    output_meta: None,
                    verbose: false
                },
                target_arg: cli::CompileTargetArg {
                    name: Some("solana".to_owned()),
                    address_length: None,
                    value_length: None
                },
                debug_features: cli::DebugFeatures {
                    log_runtime_errors: true,
                    log_prints: true,
                    generate_debug_info: false,
                    release: false,
                    strict_soroban_types: false,
                },
                optimizations: cli::Optimizations {
                    dead_storage: true,
                    constant_folding: true,
                    strength_reduce: true,
                    vector_to_slice: true,
                    common_subexpression_elimination: true,
                    opt_level: Some("3".to_owned()),
                    #[cfg(feature = "wasm_opt")]
                    wasm_opt_passes: None
                }
            }
        );

        let command = "solang compile flipper.sol sesa.sol --config-file solang.toml --contract-authors not_sesa --target polkadot --value-length=31 --address-length=33 --no-dead-storage --no-constant-folding --no-strength-reduce --no-vector-to-slice --no-cse -O 3".split(' ');

        let matches = Cli::command().get_matches_from(command);

        let compile_matches = matches.subcommand_matches("compile").unwrap();

        compile_config.overwrite_with_matches(compile_matches);

        assert_eq!(
            compile_config,
            cli::Compile {
                configuration_file: None,
                package: cli::CompilePackage {
                    input: Some(vec![
                        PathBuf::from("flipper.sol"),
                        PathBuf::from("sesa.sol")
                    ]),
                    contracts: Some(vec!["flipper".to_owned()]),
                    import_path: Some(vec![]),
                    import_map: Some(vec![]),
                    authors: Some(vec!["not_sesa".to_owned()]),
                    version: Some("0.1.0".to_string()),
                    soroban_version: None
                },
                compiler_output: cli::CompilerOutput {
                    emit: None,
                    std_json_output: false,
                    output_directory: None,
                    output_meta: None,
                    verbose: false
                },
                target_arg: cli::CompileTargetArg {
                    name: Some("polkadot".to_owned()),
                    address_length: Some(33),
                    value_length: Some(31)
                },
                debug_features: cli::DebugFeatures {
                    log_runtime_errors: true,
                    log_prints: true,
                    generate_debug_info: false,
                    release: false,
                    strict_soroban_types: false,
                },
                optimizations: cli::Optimizations {
                    dead_storage: false,
                    constant_folding: false,
                    strength_reduce: false,
                    vector_to_slice: false,
                    common_subexpression_elimination: false,
                    opt_level: Some("3".to_owned()),
                    #[cfg(feature = "wasm_opt")]
                    wasm_opt_passes: None
                }
            }
        );
    }
}
