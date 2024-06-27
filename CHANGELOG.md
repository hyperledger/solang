# Changelog
All notable changes to [Solang](https://github.com/hyperledger/solang/)
will be documented here.

## Unreleased

## v0.3.4 Athens

### Added
- **Experimental**: Three address code format IR for the CFG has been implemented (disabled by default).
  [fanyi-zhao](https://github.com/fanyi-zhao).
- **Soroban** Work on adding support for [Stellar's Soroban](https://soroban.stellar.org/docs) contracts platforms started, by adding a skeleton that supports the Soroban runtime. [Salaheldin Soliman](https://github.com/salaheldinsoliman)
- The `string.concat()` and `bytes.concat()` builtin functions are supported. [seanyoung](https://github.com/seanyoung)
- **Polkadot**: Compatibility with ink! v5.0 metadata and substrate-contracts-node v0.39.0
- Implement event selectors [seanyoung](https://github.com/seanyoung)
- Add a feature flag for compiling language server [xermicus](https://github.com/xermicus)
- Parsing the pragma solidity version numbers, making this information available in the downstream 
  compilation pipeline [seanyoung](https://github.com/seanyoung)

### Changed
- **Polkadot/BREAKING**: Event encoding and topics follow [ink! v5.0](https://use.ink/faq/migrating-from-ink-4-to-5#events-20-1)
- **BREAKING** The non-standard extension of concatenating strings using the `+` operator
  has been removed, use `string.concat()` instead. [seanyoung](https://github.com/seanyoung)
- Removed the `--no-log-api-return-codes` compile flag as this is now done by the runtime [xermicus](https://github.com/xermicus)
- **Solana/BREAKING**: Remove balance, transfer, and send builtins from Solana [LucasSte](https://github.com/LucasSte)
- No longer support numbers in octal notation [seanyoung](https://github.com/seanyoung)
- Moved to LLVM version 16 [seanyoung](https://github.com/seanyoung)
- Improve overloaded function call diagnostics: From Solidity 0.6 onwards, overloaded functions or events
  resolving to multiple candidates are an error. In earlier versions, the first result is used.
  [seanyoung](https://github.com/seanyoung)
- **Polkadot**: Implement the caller_is_root runtime API as a builtin [xermicus](https://github.com/xermicus)

### Fixed
- Unreachable code, function types with parameter/return names and unknown assembly flags are warnings
  instead of errors, matching with solc. [xermicus](https://github.com/xermicus)
- Fixed a bunch of typos in the documentation [divdeploy](https://github.com/divdeploy)
- Clean up the LLD linker context after linking each contract, preventing potential issues when compiling
  multiple contracts at once. [xermicus](https://github.com/xermicus)
- Expression statement should be followed by a semicolon, fixing a bug where "_;" was incorrectly parsed
  as a variable. [seanyoung](https://github.com/seanyoung)
- Represent type(T) correctly in the AST, fixing various related issues [seanyoung](https://github.com/seanyoung)
- Fix a bug in abi.encodeCall() argument parsing when there is only a single argument [seanyoung](https://github.com/seanyoung)
- Fixed a codegen bug when the RHS of a shift expression is a struct member [PaddyClark0](https://github.com/PaddyClark0)


## v0.3.3 Atlantis

This release improves the Solana developer experience, since now required
accounts can be specified using annotations. For Polkadot, compatibility with
Ethereum Solidity has been increased further, it is now possible to write
[EIP-1967](https://eips.ethereum.org/EIPS/eip-1967) compatible proxy contracts.
There are many fixes all over the code base.

### Added
- **Solana** the required accounts for instructions can now be specified using function annotations. [LucasSte](https://github.com/LucasSte)
  ```
  contract Foo {
      @account(oneAccount)
      @signer(mySigner)
      @mutableAccount(otherAccount)
      @mutableSigner(otherSigner)
      function bar() external returns (uint64) {}
  }
  ```
- The language server can now format Solidity source code using the `forge-fmt` crate. [chioni16](https://github.com/chioni16)
- The langauge server can now do go references, go to implementation, and go to type
  definition. [chioni16](https://github.com/chioni16)
- **Polkadot** `Panic` errors can now be caught in try-catch statements [xermicus](https://github.com/xermicus)
- **Polkadot** custom errors are now supported [xermicus](https://github.com/xermicus)
- **Polkadot** now supporting the `address.code` builtin [xermicus](https://github.com/xermicus)

### Fixed
- **Solana** the data field of AccountInfo can now be modified. [LucasSte](https://github.com/LucasSte)
- The vscode extension now uses the solang binary in the path, if available. [seanyoung](https://github.com/seanyoung)
- Fixed a bug in the ABI encoding of dynamic arrays. [xermicus](https://github.com/xermicus)
- Fixed a bug where loading from a storage struct member was not considered a storage read.
  [xermicus](https://github.com/xermicus) [seanyoung](https://github.com/seanyoung)
- Fixed a compiler crash caused by chained assignments like `return a = b`.  [xermicus](https://github.com/xermicus)
- Variables declared in the return parameters no longer ignore the `storage` location. [xermicus](https://github.com/xermicus)

### Changed
- **BREAKING:** **Solana** the contract Solidity type can no longer be used. This type
  used to represent a single address, but this does not make sense as there are many
  accounts associated with a contract call. [LucasSte](https://github.com/LucasSte)

## v0.3.2 Brasília

The language server is much improved, and many fixes all over.

### Added
- Go to definition is now implemented in the language server. [chioni16](https://github.com/chioni16)
- The parser has been updated to be compatible with Ethereum Solidity v0.8.21. [seanyoung](https://github.com/seanyoung)
- **Polkadot** Support for runtimes built with on `polkadot-v1.0.0`

### Fixed
- **breaking** Resolving import paths now matches solc more closely, and only resolves relative
  paths when specified as `./foo` or `../foo`. [seanyoung](https://github.com/seanyoung)
- **Solana** The `lamports` and `data` fields of `tx.accounts` can be modified again. [LucasSte](https://github.com/LucasSte)
- **Solana** The `address.transfer()` and `address.send()` functions no longer change any balances
  in the error case if there was an overflow (e.g. not enough balance).
  [LucasSte](https://github.com/LucasSte)
- **Solana** When collecting the required accounts, ensure that the writer and signer bits are set
  correctly if the same account is used multiple times. [LucasSte](https://github.com/LucasSte)
- It is not longer necessary to save a Solidity file, in order for the language server to pick
  up changes to the file. [chioni16](https://github.com/chioni16)
- The negate operator `-` now checks for overflow at runtime, and other math overflow fixes.
  [seanyoung](https://github.com/seanyoung)
- Fixed a bug where accessing the function selector of virtual functions might cause a compiler panic. [xermicus](https://github.com/xermicus)
- Fixed a bug where the strength reduce optimization pass removed overflow checks on optimized multiplications. [xermicus](https://github.com/xermicus)
- Fixed a bug where external function variables were not marked as `read` when they were called by the semantic analyzer, which could lead to the external function call being eliminated spuriously. [xermicus](https://github.com/xermicus)
- Fixed a bug in `try-catch` where a failed transfer trapped the contract instead of handling it in a catch all block. [xermicus](https://github.com/xermicus)

### Changed
- The Substrate target has been renamed to Polkadot. [xermicus](https://github.com/xermicus)
- **Polkadot** `assert()` and `require()` is now implemented as a transction revert, rather
  than a trap. The error data is returned, and encoded the same as on Ethereum. Error data is now
  passed to the calling contract, all the way up the call stack. [xermicus](https://github.com/xermicus)
- **Polkadot** constructor can be non-payable. [xermicus](https://github.com/xermicus)
- Storage variables are now always written to, regardless whether the contract does read them or not.
  Prior behavior was to not write to storage variables if they are not read, which can remove wanted
  side effects, because unused storage variables may be used in future versions of the contract. [xermicus](https://github.com/xermicus)
- **Solana** seeds can now be of type `address` or `bytesN`, in addition to `bytes`. [seanyoung](https://github.com/seanyoung)

## v0.3.1 Göttingen

### Added
- Write environment configuration into Substrate metadata. [xermicus](https://github.com/xermicus)
- Tornado cash as an exemplary integration test for Substrate chain extensions. [xermicus](https://github.com/xermicus)
- `is_contract` runtime API is available as a builtin for Substrate. [xermicus](https://github.com/xermicus)
- The `wasm-opt` optimizer now optimizes the Wasm bytecode on the Substrate target. [xermicus](https://github.com/xermicus)
- Call flags are now available for Substrate. [xermicus](https://github.com/xermicus)
- Read compiler configurations from toml file. [salaheldinsoliman](https://github.com/salaheldinsoliman)
- Accounts declared with `@payer(my_account)` can be accessed with the
  syntax `tx.accounts.my_account`. [LucasSte](https://github.com/LucasSte)
- `delegatecall()` builtin has been added for Substrate. [xermicus](https://github.com/xermicus)
- `get_contents_of_file_no` for Solang parser. [BenTheKush](https://github.com/BenTheKush)
- `set_code_hash()` builtin has been aded for Substrate. [xermicus](https://github.com/xermicus)

### Fixed
- Diagnostics do not include large numbers anymore. [seanyoung](https://github.com/seanyoung)
- Correctly parse and resolve `require(i < 2**255)`. [seanyoung](https://github.com/seanyoung)
- Virtual function are available for call. [xermicus](https://github.com/xermicus)
- Allow `.wrap()` and `.unwrap()` user defined type methods in constants. [xermicus](https://github.com/xermicus)
- `@inheritdoc` now works with bases of bases. [seanyoung](https://github.com/seanyoung)
- Allow destructures to assign to storage variables. [seanyoung](https://github.com/seanyoung)
- Do not allow push and pop in fixed length arrays. [LucasSte](https://github.com/LucasSte)
- Improve unused variable elimination to remove unused arrays. [LucasSte](https://github.com/LucasSte)
- Salt argument should be of type `bytes32`. [seanyoung](https://github.com/seanyoung)
- Allow return vallues to be ignored in try-catch statements. [seanyoung](https://github.com/seanyoung)
- Optimize modifiers' CFGs. [xermicus](https://github.com/xermicus)
- Fix an error whereby building large contracts would cause an LLVM error. [LucasSte](https://github.com/LucasSte)
- A constructor for a Solana contract cannot run twice on the same data account. [seanyoung](https://github.com/seanyoung)
- Split the `call` and `deploy` dispatches on Substrate. [xermicus](https://github.com/xermicus)

### Changed
-  Minimum Supported Rust Version (MSRV) is Rust `1.68`.
- `@payer` annotation declares an account in a constructor. [LucasSte](https://github.com/LucasSte)
- Do not allow `.call()` functions in functions declared as view. [seanyoung](https://github.com/seanyoung)
- Storage accessor function matches solc, and returns struct members if the sole return value is a single struct [seanyoung](https://github.com/seanyoung)
- **breaking** Constructor annotations above a constructor can either declare an account or receive a literal parameter. [LucasSte](https://github.com/LucasSte)
  ```
  contract MyContract {
    @payer(acc) // Declares account acc
    @space(2+3) // Only literals or constant expressions allowed
    @seed(myseed) // NOT ALLOWED
    constructor(bytes myseed) {}
  }
  ```
- Annotations placed before constructor arguments refer to the latter. [LucasSte](https://github.com/LucasSte)
  ```
  contract MyContract {
    @payer(acc) // Declares account acc
    @space(2+3) // Only literals or constant expressions allowed
    constructor(@seed bytes myseed) {}
    // When an annotations refers to a parameter, the former must appear right before the latter.
  }
  ```

## v0.3.0 Venice

The parser and semantic analysis stage of Solang have gone through
[a security audit](https://github.com/solana-labs/security-audits/blob/master/solang/Trail_of_Bits_Solang_Final_report.pdf). All security issues have been fixed.

### Added
- The CLI now has a `--release` option, which disables printing of errors [salaheldinsoliman](https://github.com/salaheldinsoliman)
- **Substrate**: chain extensions can be now used.
  [xermicus](https://github.com/xermicus)

### Fixed
- Solidity error definitions are now parsed.
  [seanyoung](https://github.com/seanyoung)
- The Ethereum Solidity parser and semantic analysis tests are now run on Solang sema during
  `cargo test`.
  [seanyoung](https://github.com/seanyoung)
- If a function returns a `storage` reference, then not returning a value explicitly is an error, since
  the reference must refer to an existing storage variable.
  [seanyoung](https://github.com/seanyoung)
- Many small improvements have been made to the parser and semantic analysis, improving compatibility
  with Ethereum Solidity.
  [seanyoung](https://github.com/seanyoung)
  [xermicus](https://github.com/xermicus)
  [LucasSte](https://github.com/LucasSte)

### Changed
- **Solana**: Addresses are now base58 encoded when formated with `"address:{}".format(address)`.
  [LucasSte](https://github.com/LucasSte)
- **Substrate**: No longer use the prefixed names for seal runtime API calls, which grants small improvements in contract sizes. [xermicus](https://github.com/xermicus)

## v0.2.3 Geneva

### Added
- The Solana units `sol` and `lamports` are now supported, e.g. `10 sol` and `100 lamports`.
  [seanyoung](https://github.com/seanyoung)
- User defined operators are now supported. This is a feature in Ethereum Solidity v0.8.19.
  [seanyoung](https://github.com/seanyoung)
- **Solana**: if a contract uses the `SystemAccount`, `ClockAccount`, or other standard builtin
  accounts, then this is automatically added to the IDL. [LucasSte](https://github.com/LucasSte)
- **Substrate**: The content of the debug buffer is formatted in a human readable way. This vastly improves its readability, allowing to spot API runtime return codes, runtime errors and debug prints much easier. [salaheldinsoliman](https://github.com/salaheldinsoliman)

### Fixed
- Solana: contracts with a seed for the constructor do not require a signer in the Anchor IDL
  [seanyoung](https://github.com/seanyoung)
- Fix panic when lexing ".9" at the beginning of a file. [seanyoung](https://github.com/seanyoung)
- Forbid ABI encoding and decoding of recursive types. [xermicus](https://github.com/xermicus)
- Treat enums as 8bit uint in constant hashing. [xermicus](https://github.com/xermicus)
- Fix compilation failure with -g for the substrate target. [salaheldinsoliman](https://github.com/salaheldinsoliman)
- Fixed incorrect ABI encoding for user defined types. [xermicus](https://github.com/xermicus)  [seanyoung](https://github.com/seanyoung)
- Fixed incorrect ABI encoding for struct with fields of type `bytesN` [xermicus](https://github.com/xermicus)
- Fixed incorrect handling of recursive struct fields. [xermicus](https://github.com/xermicus)
- Fixed a bug in our Common Subexpression Elimination optimization pass [LucasSte](https://github.com/LucasSte)
### Changed
- Math overflow is now always enabled, unless the math happens with an `unchecked { .. }` block.
  The `--math-overflow` command line option has been removed. [seanyoung](https://github.com/seanyoung)
- **Substrate**: the SCALE encoder and decoder now uses a much better implementation written in our
  CFG intermediate format. [xermicus](https://github.com/xermicus)
- **Substrate**: When instantiating a new contract without providing a salt, the salt we be derived from the output of the new `instantiation_nonce` runtime API. [xermicus](https://github.com/xermicus)
- Minimal Supported Rust Version is `1.65.0`
- No longer silently overwrite contract artifacts, if the same contract is defined more than once in different locations [seanyoung](https://github.com/seanyoung)

## v0.2.2 Alexandria

### Added
- Solidity mappings can now have named key and named value types. [seanyoung](https://github.com/seanyoung)

### Changed
- Solang now uses LLVM 15. [LucasSte](https://github.com/LucasSte)
- Solidity on Solana now required the Anchor framework for the client code, and the `@solana/solidity.js`
  Typescript library is no longer compatible with Solidity.
- When casting hex literal numbers into the `bytesN` type, the hex literal may use leading zeros to match the size
with the according `bytesN`, which aligns solang with `solc`. [xermicus](https://github.com/xermicus)

### Fixed
- Many bugs have been fixed by [seanyoung](https://github.com/seanyoung), [LucasSte](https://github.com/LucasSte)
  and [xermicus](https://github.com/xermicus)
- Typos throughout the code have been fixed. [omahs](https://github.com/omahs)

## v0.2.1 Rio

### Added
- The Anchor IDL data structure is now generated for every Solana contract, although the actual IDL json file is not yet saved.
[LucasSte](https://github.com/LucasSte)

### Changed
- The Solana target now utilizes eight byte Anchor discriminators for function dispatch instead
of the four byte Ethereum selectors. [LucasSte](https://github.com/LucasSte)
- The deployment of contracts on Solana now follows the same scheme as Anchor. [seanyoung](https://github.com/seanyoung)
- Compares between rational literals and integers are not allowed. [seanyoung](https://github.com/seanyoung)
- Overriding the function selector value is now done using the `@selector([1, 2, 3, 4])`
  syntax, and the old syntax `selector=hex"12345678"` has been removed.
- `msg.sender` was not implemented correctly on Solana, and
  [has now been removed](https://solang.readthedocs.io/en/latest/targets/solana.html#msg-sender-solana).
  [seanyoung](https://github.com/seanyoung)
- Solang now uses LLVM 14. [LucasSte](https://github.com/LucasSte)

### Fixed
- Many bugs have been fixed by [seanyoung](https://github.com/seanyoung), [LucasSte](https://github.com/LucasSte)
  and [xermicus](https://github.com/xermicus)

## v0.2.0 Berlin
We are happy to release solang `v0.2.0` codenamed `Berlin` today. Aside from
containing many small fixes and improvements, this release marks a milestone
towards maturing our Substrate compilation target: any regressions building up
since `ink!` v3.0 are fixed, most notably the metadata format (shoutout and many
thanks to external contributor [extraymond](https://github.com/extraymond)) and
event topics. Furthermore, we are leaving `ink!` version 3 behind us, in favor
of introducing compatibility with the recent `ink!` 4 beta release and the latest
substrate contracts node `v0.22.1`.

### Added
- **Solana / breaking:** The try-catch construct is no longer permitted on Solana, as it
  never worked. Any CPI error will abort the transaction.
  [seanyoung](https://github.com/seanyoung)
- **Solana:** Introduce new sub-command `solang idl` which can be used for generating
  a Solidity interface file from an Anchor IDL file. This can be used for calling
  Anchor Contracts on Solana. [seanyoung](https://github.com/seanyoung)
- **Substrate:** Provide specific Substrate builtins via a "substrate" file. The
  `Hash` type from `ink!` is the first `ink!` specific type made available for Solidity
  contracts.
  [xermicus](https://github.com/xermicus)
- **Substrate:** Introduce the `--log-api-return-codes` CLI flag, which changes the
  emitted code to print return codes for `seal` API calls into the debug buffer.
  [xermicus](https://github.com/xermicus)
- Introduce function name mangling for overloaded functions and constructors, so
  that they can be represented properly in the metadata.
  [xermicus](https://github.com/xermicus)

### Changed
- The Solana target now uses Borsh encoding rather than eth abi
  encoding. This is aimed at making Solang contracts Anchor compatible.
  [LucasSte](https://github.com/LucasSte)
- **Substrate / breaking:** Supported node version is now pallet contracts `v0.22.1`.
  [xermicus](https://github.com/xermicus)
- **Substrate / breaking:** Remove the deprecated `random` builtin.
  [xermicus](https://github.com/xermicus)

### Fixed
- Whenever possible, the parser does not give up after the first error.
  [salaheldinsoliman](https://github.com/salaheldinsoliman)
- Constant expressions are checked for overflow.
  [salaheldinsoliman](https://github.com/salaheldinsoliman)
- AddMod and MulMod were broken. This is now fixed.
  [LucasSte](https://github.com/LucasSte)
- **Substrate / breaking:** Solang is now compatible with `ink!` version 4 (beta).
  [xermicus](https://github.com/xermicus)
- **Substrate:** Switched ABI generation to use official `ink!` crates, which fixes all
  remaining metadata regressions.
  [extraymond](https://github.com/extraymond) and [xermicus](https://github.com/xermicus)
- **Substrate:** Allow constructors to have a name, so that multiple constructors are
  supported, like in `ink!`.
  [xermicus](https://github.com/xermicus)
- All provided examples as well as most of the Solidity code snippets in our
  documentation are now checked for succesful compilation on the Solang CI.
  [xermicus](https://github.com/xermicus)
- **Substrate:** Fix events with topics. The topic hashes generated by Solang
  contracts are now exactly the same as those generated by `ink!`.
  [xermicus](https://github.com/xermicus)

## v0.1.13 Genoa

### Changed
- Introduce sub-commands to the CLI. Now we have dedicated sub-commands for
  `compile`, `doc`, `shell-completion` and the `language-server`, which makes
  for a cleaner CLI.
  [seanyoung](https://github.com/seanyoung)
- On Solana, emitted events are encoded with Borsh encoding following the Anchor
  format.
  [LucasSte](https://github.com/LucasSte)
- The ewasm target has been removed, since ewasm is not going to implemented on
  Ethereum. The target has been reused for an new EVM target, which is not complete
  yet.
  [seanyoung](https://github.com/seanyoung)
- Substrate: Concrete contracts must now have at least one public function. A
  public function is in a contract, if it has public or external functions, if
  it has a receive or any fallback function or if it has public storage items
  (those will yield public getters). This aligns solang up with `ink!`.
  [xermicus](https://github.com/xermicus)

### Added
- Solana v1.11 is now supported.
  [seanyoung](https://github.com/seanyoung)
- On Solana, programs now use a custom heap implementation, just like on
  Substrate. As result, it is now possible to `.push()` and `.pop()` on
  dynamic arrays in memory.
  [seanyoung](https://github.com/seanyoung)
- Arithmetic overflow tests are implemented for all integer widths,
  [salaheldinsoliman](https://github.com/salaheldinsoliman)
- Add an NFT example for Solana
  [LucasSte](https://github.com/LucasSte)
- Add a wrapper for the Solana System Program
  [LucasSte](https://github.com/LucasSte)
- The selector for functions can be overriden with the `selector=hex"abcd0123"`
  syntax.
  [seanyoung](https://github.com/seanyoung)
- Shell completion is available using the `solang shell-completion` subcommand.
  [xermicus](https://github.com/xermicus)
- Add support for the `create_program_address()` and `try_find_program_address()`
  system call on Solana
  [seanyoung](https://github.com/seanyoung)
- Substrate: The `print()` builtin is now supported and will write to the debug
  buffer. Additionally, error messages from the `require` statements will now be
  written to the debug buffer as well. The Substrate contracts pallet prints the
  contents of the debug buffer to the console for RPC ("dry-run") calls in case
  the `runtime::contracts=debug` log level is configured.
  [xermicus](https://github.com/xermicus)

### Fixed
- DocComments `/** ... */` are now permitted anywhere.
  [seanyoung](https://github.com/seanyoung)
- Function calls to contract functions via contract name are no longer possible,
  except for functions of base contracts.
  [xermicus](https://github.com/xermicus)

## v0.1.12 Cairo

### Added
- Added spl-token integration for Solana
- Solang now generates code for inline assembly, including many Yul builtins

### Changed
- The documentation has been re-arranged for readability.
- The solang parser can parse the same syntax as Ethereum Solidity 0.8.

### Fixed
- Fixed many parser issues. Now solang-parser parses all files in the
  Ethereum Solidity test suite. First run
  `git submodule update --init --recursive` to fetch the test files, and
  then run the test with `cargo test --workspace`.

## v0.1.11 Nuremberg

### Added
- Added support for Solidity user types
- Support `using` syntax on file scope
- Support binding functions with `using`
- Implemented parsing and semantic analysis of yul (code generation is to
  follow)
- The language server uses the `--import` and `--importmap` arguments
- On Solana, it is possible to set the accounts during CPI using the
  `accounts:` call argument.

### Fixed
- Fixed associativity of the power operator
- A huge amount of fixes improving compatibility with solc

## v0.1.10 Barcelona

### Added
- On Solana, the accounts that were passed into the transactions are listed in
  the `tx.accounts` builtin. There is also a builtin struct `AccountInfo`
- A new common subexpression elimination pass was added, thanks to
  [LucasSte](https://github.com/hyperledger/solang/pull/550)
- A graphviz dot file can be generated from the ast, using `--emit ast-dot`
- Many improvements to the solidity parser, and the parser has been spun out
  in it's own create `solang-parser`.

### Changed
- Solang now uses LLVM 13.0, based on the [Solana LLVM tree](https://github.com/solana-labs/llvm-project/)
- The ast datastructure has been simplified.
- Many bugfixes across the entire tree.

## v0.1.9

### Added
- Added support for solc import mapppings using `--importmap`
- Added support for Events on Solana
- `msg.data`, `msg.sig`, `msg.value`, `block.number`, and `block.slot` are
  implemented for Solana
- Implemented balance transfers using `.send()` and `.transfer()` on Solana
- Implemented retrieving account balances on Solana
- Verify ed25519 signatures with `signatureVerify()` on Solana
- Added support for Rational numbers
- The address type and value type can changed using `--address-length` and
  `--value-length` command line arguments (for Substrate only)

### Changed
- Solana now requires v1.8.1 or later
- On Solana, the return data is now provided in the program log. As a result,
  RPCs are now are now supported.
- On the solang command line, the target must be specified.
- The Solana instruction now includes a 64 bit value field
- Many fixes to the parser and resolver, so solidity compatibility is much
  improved, thanks to [sushi-shi](https://github.com/hyperledger/solang/pulls?q=is%3Apr+author%3Asushi-shi+is%3Aclosed).

### Removed
- The Sawtooth Sabre target has been removed.
- The generic target has been removed.

## v0.1.8

### Added
- Added a strength reduce pass to eliminate 256/128 bit multiply, division,
  and modulo where possible.
- Visual Studio Code extension can download the Solang binary from github
  releases, so the user is not required to download it themselves
- The Solana target now has support for arrays and mapping in contract
  storage
- The Solana target has support for the keccak256(), ripemd160(), and
  sha256() builtin hash functions.
- The Solana target has support for the builtins this and block.timestamp.
- Implement abi.encodePacked() for the ethereum abi encoder
- The Solana target now compiles all contracts to a single `bundle.so` BPF
  program.
- Any unused variables, events, or contract variables are now detected and
  warnings are given, thanks to [LucasSte](https://github.com/hyperledger/solang/pull/429)
- The `immutable` attribute on contract storage variables is now supported.
- The `override` attribute on public contract storage variables is now supported.
- The `unchecked {}` code block is now parsed and supported. Math overflow still
  is unsupported for types larger than 64 bit.
- `assembly {}` blocks are now parsed and give a friendly error message.
- Any variable use before it is given a value is now detected and results in
  a undefined variable diagnostic, thanks to [LucasSte](https://github.com/hyperledger/solang/pull/468)

### Changed
- Solang now uses LLVM 12.0, based on the [Solana LLVM tree](https://github.com/solana-labs/llvm-project/)

### Fixed
- Fix a number of issues with parsing the uniswap v2 contracts
- ewasm: staticcall() and delegatecall() cannot take value argument
- Fixed array support in the ethereum abi encoder and decoder
- Fixed issues in arithmetic on non-power-of-2 types (e.g. uint112)

## v0.1.7

### Added
- Added a constant folding optimization pass to improve codegen. When variables fold
  to constant values, they are visible in the hover in the extension
- For Substrate and Solana, address literals can specified with their base58 notation, e.g.
  `address foo = address"5GBWmgdFAMqm8ZgAHGobqDqX6tjLxJhv53ygjNtaaAn3sjeZ";`
- Solana account storage implemented for ``bytes``, ``string``, and structs
- Implemented ``delete`` for Solana

### Changed
- The Substrate target produces a single .contract file
- The Substrate target now uses the salt argument for seal\_instantiate()

### Fixed
- Libraries are allowed to have constant variables
- Fixed ethereum abi encoding/decoding of structs and enums
- Solana now returns an error if account data is not large enough
- Fixed storage bytes push() and pop()
- Ewasm uses precompiles for keccak hashing
- Various ewasm fixes for Hyperledger Burrow

## v0.1.6

### Added
- New Visual Studio Code extension developed under Hyperledger Mentorship
  programme
- Added language server for use in vscode extension
- Implemented primitives types and operations for Solana
- Functions can be declared outside of contracts
- Constants can be declared outside of contracts
- String formatting using python style "..{}..".format(n)

## v0.1.5

### Added
- Function types are implemented
- An experimental [Solana](https://solana.com/) target has been added
- Binaries are generated for Mac

### Changed
- The Substrate target requires Substrate 2.0

## v0.1.4

### Added
- `event` can be declared and emitted with `emit`
- Function modifiers have been implemented
- Tags in doc comments are parsed and resolved
- All major Solidity language features implemented, see our language status page:
  https://solang.readthedocs.io/en/latest/status.html

## v0.1.3

### Added
- `import` directives are supported
- New `--importpath` command line argument to specify directories to search for imports
- Contracts can have base contracts
- Contracts can be abstract
- Interfaces are supported
- Libraries are supported
- The `using` library `for` type syntax is supported

### Changed
- Solang now uses llvm 10.0 rather than llvm 8.0
- In line with Solidity 0.7.0, constructors no longer need a visibility argument