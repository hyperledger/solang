import * as StellarSdk from '@stellar/stellar-sdk';
import { readFileSync } from 'fs';
import { expect } from 'chai';
import path from 'path';
import { fileURLToPath } from 'url';
import { call_contract_function } from './test_helpers.js';  // Helper to interact with the contract

const __filename = fileURLToPath(import.meta.url);
const dirname = path.dirname(__filename);

describe('StorageTypes', () => {
  let keypair;
  const server = new StellarSdk.SorobanRpc.Server(
    "https://soroban-testnet.stellar.org:443",
  );

  let contractAddr;
  let contract;
  before(async () => {
    console.log('Setting up storage_types contract tests...');

    // Read secret from file
    const secret = readFileSync('alice.txt', 'utf8').trim();
    keypair = StellarSdk.Keypair.fromSecret(secret);

    let contractIdFile = path.join(dirname, '.soroban', 'contract-ids', 'storage_types.txt');
    // Read contract address from file
    contractAddr = readFileSync(contractIdFile, 'utf8').trim().toString();

    // Load contract
    contract = new StellarSdk.Contract(contractAddr);

    // Initialize the contract if necessary (adapt according to your contract logic)
    await call_contract_function("init", server, keypair, contract);
  });

  it('check initial values', async () => {
    // Check initial values of all storage variables
    let sesa = await call_contract_function("sesa", server, keypair, contract);
    expect(sesa.toString()).eq("1");

    let sesa1 = await call_contract_function("sesa1", server, keypair, contract);
    expect(sesa1.toString()).eq("1");

    let sesa2 = await call_contract_function("sesa2", server, keypair, contract);
    expect(sesa2.toString()).eq("2");

    let sesa3 = await call_contract_function("sesa3", server, keypair, contract);
    expect(sesa3.toString()).eq("2");
  });

  it('increment values', async () => {
    // Increment all values by calling the inc function
    await call_contract_function("inc", server, keypair, contract);

    // Check the incremented values
    let sesa = await call_contract_function("sesa", server, keypair, contract);
    expect(sesa.toString()).eq("2");

    let sesa1 = await call_contract_function("sesa1", server, keypair, contract);
    expect(sesa1.toString()).eq("2");

    let sesa2 = await call_contract_function("sesa2", server, keypair, contract);
    expect(sesa2.toString()).eq("3");

    let sesa3 = await call_contract_function("sesa3", server, keypair, contract);
    expect(sesa3.toString()).eq("3");
  });

  it('decrement values', async () => {
    // Decrement all values by calling the dec function
    await call_contract_function("dec", server, keypair, contract);

    // Check the decremented values
    let sesa = await call_contract_function("sesa", server, keypair, contract);
    expect(sesa.toString()).eq("1");

    let sesa1 = await call_contract_function("sesa1", server, keypair, contract);
    expect(sesa1.toString()).eq("1");

    let sesa2 = await call_contract_function("sesa2", server, keypair, contract);
    expect(sesa2.toString()).eq("2");

    let sesa3 = await call_contract_function("sesa3", server, keypair, contract);
    expect(sesa3.toString()).eq("2");
  });
});
