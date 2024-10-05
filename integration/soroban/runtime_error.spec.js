import * as StellarSdk from '@stellar/stellar-sdk';
import { readFileSync } from 'fs';
import { expect } from 'chai';
import path from 'path';
import { fileURLToPath } from 'url';
import { call_contract_function } from './test_helpers.js';

const __filename = fileURLToPath(import.meta.url);
const dirname = path.dirname(__filename);

describe('Runtime Error', () => {
  let keypair;
  const server = new StellarSdk.SorobanRpc.Server(
    "https://soroban-testnet.stellar.org:443",
  );

  let contractAddr;
  let contract;
  before(async () => {

    console.log('Setting up runtime_error.sol contract tests...');

    // read secret from file
    const secret = readFileSync('alice.txt', 'utf8').trim();
    keypair = StellarSdk.Keypair.fromSecret(secret);

    let contractIdFile = path.join(dirname, '.soroban', 'contract-ids', 'Error.txt');
    // read contract address from file
    contractAddr = readFileSync(contractIdFile, 'utf8').trim().toString();

    // load contract
    contract = new StellarSdk.Contract(contractAddr);

    // initialize the contract
    await call_contract_function("init", server, keypair, contract);

    // call decrement once. The second call however will result in a runtime error
    await call_contract_function("decrement", server, keypair, contract);
  });

  it('get correct initial counter', async () => {

    // decrement the counter again, resulting in a runtime error
    let res = await call_contract_function("decrement", server, keypair, contract);

    expect(res).to.contain('runtime_error: math overflow in runtime_error.sol:6:9-19');
  });

});


