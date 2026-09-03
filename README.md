<img src="https://raw.githubusercontent.com/hyperledger/solang/main/docs/hl_solang_horizontal-color.svg" alt="Solang Logo" width="75%"/>

# solang - Solidity Compiler for Solana, Polkadot and Soroban

[![Discord](https://img.shields.io/discord/905194001349627914?logo=Hyperledger&style=plastic)](https://discord.gg/hyperledger)
[![CI](https://github.com/hyperledger-solang/solang/workflows/test/badge.svg)](https://github.com/hyperledger-solang/solang/actions)
[![Documentation Status](https://readthedocs.org/projects/solang/badge/?version=latest)](https://solang.readthedocs.io/en/latest/?badge=latest)
[![license](https://img.shields.io/github/license/hyperledger/solang.svg)](LICENSE)
[![LoC](https://tokei.rs/b1/github/hyperledger/solang?category=lines)](https://github.com/hyperledger-solang/solang)

Welcome to Solang, a new Solidity compiler written in rust which uses
llvm as the compiler backend. Solang can compile Solidity for Solana, Soroban and the
Polkadot Parachains with the `contracts` pallet.
Solang is source compatible with Solidity 0.8,
with some caveats due to differences in the underlying blockchain.

Solang is under active development right now, and has
[extensive documentation](https://solang.readthedocs.io/en/latest/).

## Solana

Please follow the [Solang Getting Started Guide](https://solang.readthedocs.io/en/latest/targets/solana.html#getting-started-on-solana).

Solang is part of the [Solana Tools Suite](https://docs.solana.com/cli/install-solana-cli-tools) (version v1.16.3 and higher).
There is no need to install it separately.

## Installation

Solang is available as a Brew cask for MacOS, with the following command:

```
brew install hyperledger/solang/solang
```

For other operating systems, please check the [installation guide](https://solang.readthedocs.io/en/latest/installing.html).

## Build for Polkadot

Run the following command, selecting the flipper example available on Solang's repository:

```bash
solang compile --target polkadot examples/polkadot/flipper.sol
```

Alternatively if you want to use the solang container, run:

```
docker run --rm -it -v $(pwd):/sources ghcr.io/hyperledger/solang compile -v -o /sources --target polkadot /sources/flipper.sol
```
You will have a file called flipper.contract. You can use this directly in
the [Contracts UI](https://contracts-ui.substrate.io/),
as if your smart contract was written using ink!.


## Build for Soroban

Select one of the supported contracts for Soroban, available in on Solang's repository:

```bash
solang compile --target soroban examples/soroban/token.sol
```

You will have a file called `token.wasm`. Deploy it using the [`Stellar CLI`](https://developers.stellar.org/docs/tools/cli), after following the [`Stellar CLI Setup Manual`](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup):

``` bash
stellar contract deploy --source-account alice --wasm token.wasm --network testnet -- --_admin alice --_name SolangToken --_symbol SOLT --_decimals 18
ℹ️  Skipping install because wasm already installed
ℹ️  Using wasm hash b1c84d8b8057a62fb6d77ef55c9e7fb2e66c74136c7df32efd87a1c9d475f1b0
ℹ️  Simulating deploy transaction…
ℹ️  Transaction hash is fc3b1f00d2940e646d210e6e96347fd45dc8dd873009604ec67957edb6f6589d
🔗 https://stellar.expert/explorer/testnet/tx/fc3b1f00d2940e646d210e6e96347fd45dc8dd873009604ec67957edb6f6589d
ℹ️  Signing transaction: fc3b1f00d2940e646d210e6e96347fd45dc8dd873009604ec67957edb6f6589d
🌎 Submitting deploy transaction…
🔗 https://stellar.expert/explorer/testnet/contract/CDGUMUXA6IRRVMMKIVQJWLZZONDXBJ4AITHQS757PTBVAL4U54HI3KEW
✅ Deployed!
CDGUMUXA6IRRVMMKIVQJWLZZONDXBJ4AITHQS757PTBVAL4U54HI3KEW
```

Once deployed, copy the deployed contract ID and interact with it:

``` bash
stellar contract invoke --network testnet --id CDGUMUXA6IRRVMMKIVQJWLZZONDXBJ4AITHQS757PTBVAL4U54HI3KEW  --source-account alice -- mint --to alice --amount 120
ℹ️  Signing transaction: e0d68ae85bfbe0fceed8bcadd6613e12b3159f27dbf7c18e35e94de2b4a11ee2
```

For the current Soroban support matrix, see the
[`Soroban support matrix`](https://solang.readthedocs.io/en/latest/targets/soroban_support_matrix.html).
For Soroban compatibility details, see the
[`Soroban target docs`](https://solang.readthedocs.io/en/latest/targets/soroban.html).
The repository-level Soroban maintenance policy lives in [`MAINTENANCE.MD`](MAINTENANCE.MD).



## Tentative roadmap

Solang has a high level of compatibility with many blockchains. We are trying to ensure the compiler stays
up to date with the newest Solidity syntax and features.  In addition, we focus on bringing new performance optimizations
and improve developer experience.
Here is a brief description of what we envision for the next versions.

### V0.4

| Feature                                            | Status                                               |
|----------------------------------------------------|------------------------------------------------------|
| Improve management over optimization passes        | Not started                                          |
| Adopt single static assignment for code generation | In progress                                          |
| Support openzeppelin on Polkadot target            | In progress                                          |
| Provide Solidity -> Polkadot porting guide         | Not started                                          |
| Declare accounts for a Solidity function on Solana | In progress                                          |
| Tooling for calls between ink! <> solidity         | In progress                                          |
| Provide CLI for node interactions                  | [Done](https://github.com/hyperledger-solang/solang-aqd)    |
| Support all [Soroban examples](https://github.com/stellar/soroban-examples) | In progress |

## License

[Apache 2.0](LICENSE)
