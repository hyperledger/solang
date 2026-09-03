// SPDX-License-Identifier: Apache-2.0

// Disclaimer: This library provides a way for Solidity to interact with Solana's SPL-Token. Although it is production ready,
// it has not been audited for security, so use it at your own risk.

import 'solana';

library SplToken {
	address constant tokenProgramId = address"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
	enum TokenInstruction {
		InitializeMint, // 0
		InitializeAccount, // 1
		InitializeMultisig, // 2
		Transfer, // 3
		Approve, // 4
		Revoke, // 5
		SetAuthority, // 6
		MintTo, // 7
		Burn, // 8
		CloseAccount, // 9
		FreezeAccount, // 10
		ThawAccount, // 11
		TransferChecked, // 12
		ApproveChecked, // 13
		MintToChecked, // 14
		BurnChecked, // 15
		InitializeAccount2, // 16
		SyncNative, // 17
		InitializeAccount3, // 18
		InitializeMultisig2, // 19
		InitializeMint2, // 20
		GetAccountDataSize, // 21
		InitializeImmutableOwner, // 22
		AmountToUiAmount, // 23
		UiAmountToAmount, // 24
		InitializeMintCloseAuthority, // 25
		TransferFeeExtension, // 26
		ConfidentialTransferExtension, // 27
		DefaultAccountStateExtension, // 28
		Reallocate, // 29
		MemoTransferExtension, // 30
		CreateNativeMint // 31
	}

	/// Mint new tokens. The transaction should be signed by the mint authority keypair
	///
	/// @param mint the account of the mint
	/// @param account the token account where the minted tokens should go
	/// @param authority the public key of the mint authority
	/// @param amount the amount of tokens to mint
	function mint_to(address mint, address account, address authority, uint64 amount) internal {
		bytes instr = new bytes(9);

		instr[0] = uint8(TokenInstruction.MintTo);
		instr.writeUint64LE(amount, 1);

		AccountMeta[3] metas = [
			AccountMeta({pubkey: mint, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: account, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: authority, is_writable: false, is_signer: true})
		];

		tokenProgramId.call{accounts: metas}(instr);
	}

	/// Transfer @amount token from @from to @to. The transaction should be signed by the owner
	/// keypair of the from account.
	///
	/// @param from the account to transfer tokens from
	/// @param to the account to transfer tokens to
	/// @param owner the publickey of the from account owner keypair
	/// @param amount the amount to transfer
	function transfer(address from, address to, address owner, uint64 amount) internal {
		bytes instr = new bytes(9);

		instr[0] = uint8(TokenInstruction.Transfer);
		instr.writeUint64LE(amount, 1);

		AccountMeta[3] metas = [
			AccountMeta({pubkey: from, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: to, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: owner, is_writable: false, is_signer: true})
		];

		tokenProgramId.call{accounts: metas}(instr);
	}

	/// Transfer @amount token from @from (PDA) to @to. The transaction will be signed by PDA account
	function transfer_pda(address pda_token_account, address to, address pda, uint64 amount, bytes bump) internal {
		bytes instr = new bytes(9);

		instr[0] = uint8(TokenInstruction.Transfer);
		instr.writeUint64LE(amount, 1);

		AccountMeta[3] metas = [
			AccountMeta({pubkey: pda_token_account, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: to, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: pda, is_writable: false, is_signer: true})
		];
		tokenProgramId.call{accounts: metas, seeds: [["msq", bump]]}(instr);
	}

	/// Burn @amount tokens in account. This transaction should be signed by the owner.
	///
	/// @param account the acount for which tokens should be burned
	/// @param mint the mint for this token
	/// @param owner the publickey of the account owner keypair
	/// @param amount the amount to burn
	function burn(address account, address mint, address owner, uint64 amount) internal {
		bytes instr = new bytes(9);

		instr[0] = uint8(TokenInstruction.Burn);
		instr.writeUint64LE(amount, 1);

		AccountMeta[3] metas = [
			AccountMeta({pubkey: account, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: mint, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: owner, is_writable: false, is_signer: true})
		];

		tokenProgramId.call{accounts: metas}(instr);
	}

	/// Approve an amount to a delegate. This transaction should be signed by the owner
	///
	/// @param account the account for which a delegate should be approved
	/// @param delegate the delegate publickey
	/// @param owner the publickey of the account owner keypair
	/// @param amount the amount to approve
	function approve(address account, address delegate, address owner, uint64 amount) internal {
		bytes instr = new bytes(9);

		instr[0] = uint8(TokenInstruction.Approve);
		instr.writeUint64LE(amount, 1);

		AccountMeta[3] metas = [
			AccountMeta({pubkey: account, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: delegate, is_writable: false, is_signer: false}),
			AccountMeta({pubkey: owner, is_writable: false, is_signer: true})
		];

		tokenProgramId.call{accounts: metas}(instr);
	}

	/// Revoke a previously approved delegate. This transaction should be signed by the owner. After
	/// this transaction, no delegate is approved for any amount.
	///
	/// @param account the account for which a delegate should be approved
	/// @param owner the publickey of the account owner keypair
	function revoke(address account, address owner) internal {
		bytes instr = new bytes(1);

		instr[0] = uint8(TokenInstruction.Revoke);

		AccountMeta[2] metas = [
			AccountMeta({pubkey: account, is_writable: true, is_signer: false}),
			AccountMeta({pubkey: owner, is_writable: false, is_signer: true})
		];

		tokenProgramId.call{accounts: metas}(instr);
	}

	/// Get the total supply for the mint, i.e. the total amount in circulation
	/// @param account The AccountInfo struct for the mint account
	function total_supply(AccountInfo account) internal view returns (uint64) {
	
		return account.data.readUint64LE(36);
	}

	/// Get the balance for an account.
	///
	/// @param account the struct AccountInfo whose account balance we want to retrive
	function get_balance(AccountInfo account) internal view returns (uint64) {

		return account.data.readUint64LE(64);
	}

	/// Get the account info for an account. This walks the transaction account infos
	/// and find the account info, or the transaction fails.
	///
	/// @param account the account for which we want to have the acount info.
	function get_account_info(address account) internal view returns (AccountInfo) {
		for (uint64 i = 0; i < tx.accounts.length; i++) {
			AccountInfo ai = tx.accounts[i];
			if (ai.key == account) {
				return ai;
			}
		}

		revert("account missing");
	}

	/// This enum represents the state of a token account
	enum AccountState {
		Uninitialized,
		Initialized,
		Frozen
	}

	/// This struct is the return of 'get_token_account_data'
	struct TokenAccountData {
		address mintAccount;
		address owner;
		uint64 balance;
		bool delegate_present;
		address delegate;
		AccountState state;
		bool is_native_present;
		uint64 is_native;
		uint64 delegated_amount;
		bool close_authority_present;
		address close_authority;
	}

	/// Fetch the owner, mint account and balance for an associated token account.
	///
	/// @param ai the AccountInfo struct for the token account
	/// @return struct TokenAccountData
	function get_token_account_data(AccountInfo ai) public pure returns (TokenAccountData) {
		
		TokenAccountData data = TokenAccountData(
			{
				mintAccount: ai.data.readAddress(0), 
				owner: ai.data.readAddress(32),
			 	balance: ai.data.readUint64LE(64),
				delegate_present: ai.data.readUint32LE(72) > 0,
				delegate: ai.data.readAddress(76),
				state: AccountState(ai.data[108]),
				is_native_present: ai.data.readUint32LE(109) > 0,
				is_native: ai.data.readUint64LE(113),
				delegated_amount: ai.data.readUint64LE(121),
				close_authority_present: ai.data.readUint32LE(129) > 0,
				close_authority: ai.data.readAddress(133)
			}
		);

		return data;
	}

	// This struct is the return of 'get_mint_account_data'
	struct MintAccountData {
		bool authority_present;
		address mint_authority;
		uint64 supply;
		uint8 decimals;
		bool is_initialized;
		bool freeze_authority_present;
		address freeze_authority;
	}

	/// Retrieve the information saved in a mint account
	///
	/// @param ai the AccountInfo struct for the mint accounts
	/// @return the MintAccountData struct
	function get_mint_account_data(AccountInfo ai) public pure returns (MintAccountData) {

		uint32 authority_present = ai.data.readUint32LE(0);
		uint32 freeze_authority_present = ai.data.readUint32LE(46);
		MintAccountData data = MintAccountData( {
			authority_present: authority_present > 0,
			mint_authority: ai.data.readAddress(4),
			supply: ai.data.readUint64LE(36),
			decimals: uint8(ai.data[44]),
			is_initialized: ai.data[45] > 0,
			freeze_authority_present: freeze_authority_present > 0,
			freeze_authority: ai.data.readAddress(50)
		});

		return data;
	}

	// A mint account has an authority, whose type is one of the members of this struct.
	enum AuthorityType {
		MintTokens,
		FreezeAccount,
		AccountOwner,
		CloseAccount
	}

	/// Remove the mint authority from a mint account
	///
	/// @param mintAccount the public key for the mint account
	/// @param mintAuthority the public for the mint authority
	function remove_mint_authority(address mintAccount, address mintAuthority) public {
		AccountMeta[2] metas = [
			AccountMeta({pubkey: mintAccount, is_signer: false, is_writable: true}),
			AccountMeta({pubkey: mintAuthority, is_signer: true, is_writable: false})
		];

		bytes data = new bytes(3);
		data[0] = uint8(TokenInstruction.SetAuthority);
		data[1] = uint8(AuthorityType.MintTokens);
		data[2] = 0;
		
		tokenProgramId.call{accounts: metas}(data);
	}
}
