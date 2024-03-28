import expect from 'expect';
import { weight, createConnection, deploy, transaction, aliceKeypair, query, } from './index';
import { ContractPromise } from '@polkadot/api-contract';
import { ApiPromise } from '@polkadot/api';
import { KeyringPair } from '@polkadot/keyring/types';
import { DecodedEvent } from '@polkadot/api-contract/types';
import { AccountId, ContractSelector } from '@polkadot/types/interfaces';

interface IMsg {
    identifier: string,
    selector: any,
};

describe('Deploy the upgradable proxy and implementations; expect the upgrade mechanism to work', () => {
    // Helper: Upgrade implementation and execute a constructor that takes no arguments
    async function upgrade_version(impl: AccountId, input: any) {
        const params = [impl, input];
        const gasLimit = await weight(conn, proxy, 'upgradeToAndCall', params);
        let result: any = await transaction(proxy.tx.upgradeToAndCall({ gasLimit }, ...params), aliceKeypair());

        let events: DecodedEvent[] = result.contractEvents;
        console.log(events);
        expect(events.length).toEqual(1);
        expect(events[0].event.identifier).toBe("Upgraded");
        expect(events[0].args.map(a => a.toJSON())[0]).toEqual(params[0].toJSON());
    }

    let conn: ApiPromise;
    let alice: KeyringPair;
    let proxy: ContractPromise;
    let counter: ContractPromise;

    before(async function () {
        alice = aliceKeypair();
        conn = await createConnection();

        const proxy_deployment = await deploy(conn, alice, 'UpgradeableProxy.contract', 0n);
        proxy = new ContractPromise(conn, proxy_deployment.abi, proxy_deployment.address);

        // Pretend the proxy contract to be implementation V1
        const implV1 = await deploy(conn, alice, 'UpgradeableImplV1.contract', 0n);
        const selector = implV1.abi.messages.find((m: IMsg) => m.identifier === "inc").selector;
        await upgrade_version(implV1.address, selector);
        counter = new ContractPromise(conn, implV1.abi, proxy_deployment.address);
        const count = await query(conn, alice, counter, "count");
        expect(BigInt(count.output?.toString() ?? "")).toStrictEqual(1n);
    });

    after(async function () {
        await conn.disconnect();
    });

    it('Tests implementation and upgrading', async function () {
        // Test implementation V1
        let gasLimit = await weight(conn, counter, 'inc', []);
        await transaction(counter.tx.inc({ gasLimit }), alice);
        await transaction(counter.tx.inc({ gasLimit }), alice);
        let count = await query(conn, alice, counter, "count");
        expect(BigInt(count.output?.toString() ?? "")).toStrictEqual(3n);

        // Upgrade to implementation V2
        const implV2 = await deploy(conn, alice, 'UpgradeableImplV2.contract', 0n);
        const selector = implV2.abi.messages.find((m: IMsg) => m.identifier === "setVersion").selector;
        await upgrade_version(implV2.address, selector);
        counter = new ContractPromise(conn, implV2.abi, proxy.address);

        // Test implementation V2
        count = await query(conn, alice, counter, "count");
        expect(BigInt(count.output?.toString() ?? "")).toStrictEqual(3n);

        gasLimit = await weight(conn, counter, 'dec', []);
        await transaction(counter.tx.dec({ gasLimit }), alice);
        count = await query(conn, alice, counter, "count");
        expect(BigInt(count.output?.toString() ?? "")).toStrictEqual(2n);

        const version = await query(conn, alice, counter, "version");
        expect(version.output?.toString()).toStrictEqual("v2");
    });
});
