/*!
Fungible Token implementation with JSON serialization.
NOTES:
  - The maximum balance value is limited by U128 (2**128 - 1).
  - JSON calls should pass U128 as a base-10 string. E.g. "100".
  - The contract optimizes the inner trie structure by hashing account IDs. It will prevent some
    abuse of deep tries. Shouldn't be an issue, once NEAR clients implement full hashing of keys.
  - The contract tracks the change in storage before and after the call. If the storage increases,
    the contract requires the caller of the contract to attach enough deposit to the function call
    to cover the storage cost.
    This is done to prevent a denial of service attack on the contract by taking all available storage.
    If the storage decreases, the contract will issue a refund for the cost of the released storage.
    The unused tokens from the attached deposit are also refunded, so it's safe to
    attach more deposit than required.
  - To prevent the deployed contract from being modified or deleted, it should not have any access
    keys on its account.
*/
use near_contract_standards::fungible_token::metadata::{
    FungibleTokenMetadata, FungibleTokenMetadataProvider, FT_METADATA_SPEC,
};
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LazyOption;
use near_sdk::json_types::U128;
use near_sdk::{env, log, near_bindgen, AccountId, Balance, PanicOnDefault, PromiseOrValue};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    token: FungibleToken,
    metadata: LazyOption<FungibleTokenMetadata>,
}

const DATA_IMAGE_SVG_NEAR_ICON: &str = "data:image/svg+xml;base64,PD94bWwgdmVyc2lvbj0iMS4wIiBzdGFuZGFsb25lPSJubyI/Pgo8IURPQ1RZUEUgc3ZnIFBVQkxJQyAiLS8vVzNDLy9EVEQgU1ZHIDIwMDEwOTA0Ly9FTiIKICJodHRwOi8vd3d3LnczLm9yZy9UUi8yMDAxL1JFQy1TVkctMjAwMTA5MDQvRFREL3N2ZzEwLmR0ZCI+CjxzdmcgdmVyc2lvbj0iMS4wIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciCiB3aWR0aD0iNTAwLjAwMDAwMHB0IiBoZWlnaHQ9IjUwMC4wMDAwMDBwdCIgdmlld0JveD0iMCAwIDUwMC4wMDAwMDAgNTAwLjAwMDAwMCIKIHByZXNlcnZlQXNwZWN0UmF0aW89InhNaWRZTWlkIG1lZXQiPgo8bWV0YWRhdGE+CkNyZWF0ZWQgYnkgcG90cmFjZSAxLjE2LCB3cml0dGVuIGJ5IFBldGVyIFNlbGluZ2VyIDIwMDEtMjAxOQo8L21ldGFkYXRhPgo8ZyB0cmFuc2Zvcm09InRyYW5zbGF0ZSgwLjAwMDAwMCw1MDAuMDAwMDAwKSBzY2FsZSgwLjEwMDAwMCwtMC4xMDAwMDApIgpmaWxsPSIjMDAwMDAwIiBzdHJva2U9Im5vbmUiPgo8cGF0aCBkPSJNMTY1NSAzNDU2IGMtMjcgLTEzIC02MiAtMzYgLTc3IC01MiAtMzEgLTM0IC02NyAtMTA5IC02OCAtMTQxIDAKLTIzIC0yIC0yMyAtMTMwIC0yMyBsLTEzMCAwIDAgLTEyOSAwIC0xMzAgLTg1IC02MSAtODUgLTYxIDAgLTMxNCAwIC0zMTMgODUKLTYzIDg1IC02MyAwIC0xNjQgMCAtMTYzIDEwMyAxIGMxNjYgMiAxNTcgMyAxNTcgLTI2IDAgLTg2IDgzIC0xOTcgMTY2IC0yMjIKMzEgLTkgMjQxIC0xMiA4MzUgLTEyIDc1MiAwIDc5NiAxIDgzNCAxOSA4MSAzOCAxNDQgMTI3IDE0NSAyMDMgbDAgMzcgMTMzIDMKMTMyIDMgMyAxNjUgMyAxNjUgNzkgNTggODAgNTcgMCAzMTQgMCAzMTQgLTc5IDU5IC04MCA1OCAtMyAxMzAgLTMgMTMwIC0xMzIKMyBjLTExNCAyIC0xMzMgNSAtMTMzIDE5IDAgNzEgLTY2IDE2MyAtMTQ0IDE5OSBsLTUxIDI0IC03OTUgMCAtNzk1IDAgLTUwCi0yNHogbTE3ODUgLTk1NiBsMCAtNjgwIC05MzAgMCBjLTUxMSAwIC05MzMgLTEgLTkzNyAtMiAtNSAtMiAtOSAzMDQgLTExIDY4MApsLTIgNjgyIDk0MCAwIDk0MCAwIDAgLTY4MHoiLz4KPHBhdGggZD0iTTE4ODAgMjcwNSBjLTYwIC0xOSAtMTE5IC03MSAtMTU1IC0xMzYgLTI2IC00NyAtMzAgLTY0IC0zMCAtMTI5IDAKLTYyIDUgLTg0IDI3IC0xMjYgMTEyIC0yMTQgNDA5IC0xOTkgNTA1IDI1IDIyIDUzIDE5IDE1OCAtNyAyMTMgLTI3IDU4IC03OAoxMTIgLTEzMyAxMzkgLTUzIDI3IC0xNDcgMzQgLTIwNyAxNHoiLz4KPHBhdGggZD0iTTI5NjAgMjcxMSBjLTc4IC0yNCAtMTU1IC05NyAtMTg1IC0xNzQgLTIxIC01NyAtMTcgLTE2NSA5IC0yMTggMjIKLTQ3IDgwIC0xMDUgMTMxIC0xMzIgNTcgLTMxIDE4MyAtMzEgMjQwIDAgNTIgMjggMTAxIDc3IDEyOCAxMjcgMzEgNTkgMzEgMTkzCjAgMjUyIC02MCAxMTIgLTIwOSAxNzkgLTMyMyAxNDV6Ii8+CjwvZz4KPC9zdmc+Cg==";
const DECIMALS: u8 = 24;

#[near_bindgen]
impl Contract {
    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId, total_supply: U128) -> Self {
        Self::new(
            owner_id,
            U128(total_supply.0 * 10u128.pow(DECIMALS.into())),
            FungibleTokenMetadata {
                spec: FT_METADATA_SPEC.to_string(),
                name: "WhatsGPT".to_string(),
                symbol: "WGPT".to_string(),
                icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
                reference: None,
                reference_hash: None,
                decimals: DECIMALS,
            },
        )
    }

    /// Initializes the contract with the given total supply owned by the given `owner_id` with
    /// the given fungible token metadata.
    #[init]
    pub fn new(
        owner_id: AccountId,
        total_supply: U128,
        metadata: FungibleTokenMetadata,
    ) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        let mut this = Self {
            token: FungibleToken::new(b"a".to_vec()),
            metadata: LazyOption::new(b"m".to_vec(), Some(&metadata)),
        };
        this.token.internal_register_account(&owner_id);
        this.token.internal_deposit(&owner_id, total_supply.into());
        near_contract_standards::fungible_token::events::FtMint {
            owner_id: &owner_id,
            amount: &total_supply,
            memo: Some("Initial tokens supply is minted"),
        }
        .emit();
        this
    }

    fn on_account_closed(&mut self, account_id: AccountId, balance: Balance) {
        log!("Closed @{} with {}", account_id, balance);
    }

    fn on_tokens_burned(&mut self, account_id: AccountId, amount: Balance) {
        log!("Account @{} burned {}", account_id, amount);
    }
}

near_contract_standards::impl_fungible_token_core!(Contract, token, on_tokens_burned);
near_contract_standards::impl_fungible_token_storage!(Contract, token, on_account_closed);

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata(&self) -> FungibleTokenMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, Balance};

    use super::*;

    const TOTAL_SUPPLY: Balance = 1_000_000_000_000_000;

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new_default_meta(accounts(1).into(), TOTAL_SUPPLY.into());
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.ft_total_supply().0, TOTAL_SUPPLY);
        assert_eq!(contract.ft_balance_of(accounts(1)).0, TOTAL_SUPPLY);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    fn test_transfer() {
        let mut context = get_context(accounts(2));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(2).into(), TOTAL_SUPPLY.into());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(contract.storage_balance_bounds().min.into())
            .predecessor_account_id(accounts(1))
            .build());
        // Paying for account registration, aka storage deposit
        contract.storage_deposit(None, None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(2))
            .build());
        let transfer_amount = TOTAL_SUPPLY / 3;
        contract.ft_transfer(accounts(1), transfer_amount.into(), None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert_eq!(contract.ft_balance_of(accounts(2)).0, (TOTAL_SUPPLY - transfer_amount));
        assert_eq!(contract.ft_balance_of(accounts(1)).0, transfer_amount);
    }
}
