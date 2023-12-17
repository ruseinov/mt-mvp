mod receiver;
mod resolver;
mod token;

use crate::receiver::ext_mt_receiver;
use crate::resolver::ext_mt_resolver;
use crate::token::{Token, TokenId};
use crate::KeyPrefix::TokensPerOwner;
use near_sdk::borsh::BorshSerialize;
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::U128;
use near_sdk::{env, AccountId, AccountIdRef, BorshStorageKey};
use near_sdk::{require, Gas, PromiseOrValue};

pub type Balance = u128;

#[derive(BorshStorageKey, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
pub enum KeyPrefix {
    BalancesPerAccount { token_id: Vec<u8> },
    TotalSupply,
    BalancesPerToken,
    Token,
    OwnerByTokenId,
    TokensPerOwner,
    OwnerTokens,
}

pub struct MultiTokenContract {
    pub owner_id: AccountId,
    // This here is used just for bookkeeping, we don't actually do anything with it, except write
    // on mint and then read.
    pub total_supply: LookupMap<TokenId, Balance>,
    pub owner_by_id: UnorderedMap<TokenId, AccountId>,
    pub balances_per_token: UnorderedMap<TokenId, LookupMap<AccountId, u128>>,
    pub tokens_per_owner: LookupMap<AccountId, UnorderedSet<TokenId>>,
    pub next_token_id: u64,
}

// Note: approvals support has been removed for simplicity. This implementation also forgoes many
// necessary checks and optimizations as the goal is to simply verify the public API.
// That includes storage implementation as the `StorageManagement` trait is not specific to this
// standard.
impl MultiTokenContract {
    /// Creates a new MultiToken contract.
    ///
    /// # Arguments
    /// * `owner_id` - contract owner.
    pub fn new(owner_id: AccountId) -> Self {
        let total_supply = LookupMap::new(KeyPrefix::TotalSupply);
        let balances_per_token = UnorderedMap::new(KeyPrefix::BalancesPerToken);
        let owner_by_id = UnorderedMap::new(KeyPrefix::OwnerByTokenId);
        let tokens_per_owner = LookupMap::new(TokensPerOwner);
        Self {
            owner_id,
            total_supply,
            balances_per_token,
            owner_by_id,
            tokens_per_owner,
            next_token_id: 0,
        }
    }

    /// Mints a new token. Can only be called by the contract owner.
    /// The `token_id` is auto-generated.
    ///
    /// # Arguments
    /// * `token_owner_id` - owner of this token.
    /// * `supply` - total token supply.
    pub fn mt_mint(&mut self, token_owner_id: AccountId, supply: U128) -> Token {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "Unauthorized: {} != {}",
            env::predecessor_account_id(),
            self.owner_id
        );

        let supply = supply.into();

        self.next_token_id = self
            .next_token_id
            .checked_add(1)
            .expect("token_id overflow, can't mint any more tokens");

        let token_id = self.next_token_id.to_string();

        self.total_supply.insert(&token_id, &supply);
        self.owner_by_id.insert(&token_id, &token_owner_id);

        let mut new_account_balance = LookupMap::new(KeyPrefix::BalancesPerAccount {
            token_id: env::sha256(token_id.as_bytes()),
        });
        new_account_balance.insert(&token_owner_id, &supply);

        self.balances_per_token
            .insert(&token_id, &new_account_balance);

        Token {
            token_id,
            supply,
            owner_id: token_owner_id,
        }
    }

    /// Transfers a token amount from the caller's account.
    ///
    /// # Arguments
    /// * `receiver_id` - receiver account.
    /// * `token_id` - an id of the token to be transferred.
    /// * `amount` - total amount.
    /// * `memo` - an optional memo.
    ///
    /// NOTE: Perhaps we could get rid of the optional parameters like "memo" and implement this as
    /// some sort of an extension, otherwise it seems to weigh down on the core API.
    /// For simplicity this could be done as a wrapper in a different trait `mt_transfer_memo` or
    /// similar.
    pub fn mt_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        amount: U128,
        memo: Option<String>,
    ) {
        self.internal_transfer(
            env::predecessor_account_id(),
            receiver_id,
            token_id,
            amount.into(),
        );
    }

    /// Transfers a token amount from the caller's account and calls a method on receiver end.
    ///
    /// # Arguments
    /// * `receiver_id` - receiver account.
    /// * `token_id` - an id of the token to be transferred.
    /// * `amount` - total amount.
    /// * `memo` - an optional memo.
    /// * `msg`: a message that will be passed to receiving contract.
    pub fn mt_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128> {
        self.internal_transfer(
            env::predecessor_account_id(),
            receiver_id.clone(),
            token_id.clone(),
            amount.into(),
        );

        // Note: we default to no gas for simplicity. In the actual implementation this has to be
        // calculated.
        ext_mt_receiver::ext(receiver_id.clone())
            .with_static_gas(Gas::default())
            .mt_on_transfer(
                env::predecessor_account_id(),
                vec![token_id.clone()],
                vec![amount],
                msg,
            )
            .then(
                ext_mt_resolver::ext(env::current_account_id())
                    .with_static_gas(Gas::default())
                    .mt_resolve_transfer(
                        env::predecessor_account_id(),
                        receiver_id,
                        vec![token_id],
                        vec![amount],
                    ),
            )
            .into()
    }

    /// Transfer several different tokens from the caller's account.
    ///
    /// # Arguments
    /// * `receiver_id` - receiver account id.
    /// * `token_ids` - tokens to be transferred.
    /// * `amounts` - token amounts.
    /// * `memo` - an optional memo.
    ///
    /// TODO: Figure out if it's really necessary to have `Vec` of token_ids and amounts as opposed
    /// to having a `Map<TokenID, U128>`. It's not hard to validate on the backend, but the api is
    /// not amazing that way and very error-prone on the client-side. I would argue that adding a
    /// little client-side complexity for the sake of correctness is a good trade-off.
    /// NOTE: In the current implementation we create identical memos for each token.
    pub fn mt_batch_transfer(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        memo: Option<String>,
    ) {
        require!(
            token_ids.len() != amounts.len(),
            "each token must have it's corresponding amount and vice versa"
        );
        token_ids
            .into_iter()
            .zip(amounts)
            .for_each(|(token_id, amount)| {
                self.mt_transfer(receiver_id.clone(), token_id, amount, memo.clone())
            });
    }

    /// Transfers a token amount from the caller's account and calls a method on receiver end.
    ///
    /// # Arguments
    /// * `receiver_id` - receiver account.
    /// * `token_ids` - tokens to be transferred.
    /// * `amounts` - token amounts.
    /// * `memo` - an optional memo.
    /// * `msg`: a message that will be passed to receiving contract.
    pub fn mt_batch_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>> {
        self.mt_batch_transfer(
            receiver_id.clone(),
            token_ids.clone(),
            amounts.clone(),
            memo,
        );

        // Note: we default to no gas for simplicity. In the actual implementation this has to be
        // calculated.
        ext_mt_receiver::ext(receiver_id.clone())
            .with_static_gas(Gas::default())
            .mt_on_transfer(
                env::predecessor_account_id(),
                token_ids.clone(),
                amounts.clone(),
                msg,
            )
            .then(
                ext_mt_resolver::ext(env::current_account_id())
                    .with_static_gas(Gas::default())
                    .mt_resolve_transfer(
                        env::predecessor_account_id(),
                        receiver_id,
                        token_ids,
                        amounts,
                    ),
            )
            .into()
    }

    /// Returns a token.
    ///
    /// # Arguments
    /// * `token_id` - id of a token to return.
    // Note: this implementation currently does NOT return the owner_id, because it might be wise to
    // forgo it altogether, therefore making some room in storage.
    pub fn mt_token(&self, token_id: TokenId) -> Option<Token> {
        self.mt_supply(token_id.clone()).map(|supply| Token {
            owner_id: AccountIdRef::new_or_panic("not stored").into(),
            token_id,
            supply: supply.into(),
        })
    }

    /// Returns a list of tokens.
    ///
    /// # Arguments
    /// * `token_ids` - ids of tokens to return.
    // NOTE: Modified, no need to return `Option<Token>`, because `Token` already contains IDs.
    // Just don't return non-existent tokens.
    pub fn mt_token_list(&self, token_ids: Vec<TokenId>) -> Vec<Token> {
        token_ids
            .into_iter()
            .filter_map(|token_id| self.mt_token(token_id))
            .collect()
    }

    /// Returns account's balance of a given token.
    ///
    /// # Arguments
    /// * `account_id` - account to check the balance on.
    /// * `token_id` - token to check.
    pub fn mt_balance_of(&self, account_id: AccountId, token_id: TokenId) -> U128 {
        self.internal_unwrap_balance_of(&token_id, &account_id)
            .into()
    }

    /// Returns account's balances of given tokens.
    ///
    /// # Arguments
    /// * `account_id` - account to check the balance on.
    /// * `token_ids` - tokens to check.
    pub fn mt_batch_balance_of(&self, account_id: AccountId, token_ids: Vec<TokenId>) -> Vec<U128> {
        token_ids
            .into_iter()
            .map(|token_id| {
                self.internal_unwrap_balance_of(&token_id, &account_id)
                    .into()
            })
            .collect()
    }

    /// Gets the total supply of a token.
    ///
    /// # Arguments
    /// * `token_id` - a token to query total supply for.
    pub fn mt_supply(&self, token_id: TokenId) -> Option<U128> {
        self.total_supply.get(&token_id).map(|s| s.into())
    }

    /// Gets the total supply of a number of tokens.
    ///
    /// # Arguments
    /// * `token_ids` - a list of tokens to query total supply for.
    pub fn mt_batch_supply(&self, token_ids: Vec<TokenId>) -> Vec<Option<U128>> {
        token_ids
            .into_iter()
            .map(|token_id| self.mt_supply(token_id))
            .collect()
    }

    /// Get a list of all tokens (with pagination)
    ///
    /// NOTE: Thinking about the interface: Perhaps it is best to have one optional parameter
    /// instead of two, something like `mt_tokens(&self, pagination: Option<Pagination>)`. Then if
    /// one wants to use the defaults - it's much easier to do
    /// `contract.mt_tokens(None)` than `contract.mt_tokens(None, None)`.
    ///
    /// # Arguments:
    /// * `from_index` - Index to start from, defaults to 0 if not provided
    /// * `limit` - The maximum number of tokens to return
    ///
    /// returns: List of [Token]s.
    ///
    pub fn mt_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.owner_by_id
            .iter()
            .skip(from_index.unwrap_or_default().0 as usize)
            .take(limit.unwrap_or(u64::MAX) as usize)
            .map(|(token_id, owner_id)| self.get_token(owner_id, token_id))
            .collect()
    }

    /// Get list of all tokens by a given account
    ///
    /// NOTE: Same as above, perhaps the whole of pagination could be one optional param.
    /// # Arguments:
    /// * `account_id`: a valid NEAR account
    /// * `from_index` - Index to start from, defaults to 0 if not provided
    /// * `limit` - The maximum number of tokens to return
    ///
    /// returns: List of [Token]s owner by user
    ///
    pub fn mt_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        self.tokens_per_owner
            .get(&account_id)
            .map(|set| {
                set.iter()
                    .skip(from_index.unwrap_or_default().0 as usize)
                    .take(limit.unwrap_or(u64::MAX) as usize)
                    .map(|token_id| self.get_token(account_id.clone(), token_id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl MultiTokenContract {
    fn get_token(&self, owner_id: AccountId, token_id: TokenId) -> Token {
        let supply = self
            .total_supply
            .get(&token_id)
            .expect("Total supply not found by token id");

        Token {
            token_id,
            owner_id,
            supply,
        }
    }

    // Transfer tokens from one account to another.
    fn internal_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
        amount: Balance,
    ) {
        require!(sender_id != receiver_id, "Sender and receiver must differ");
        require!(amount > 0, "Transferred amounts must be greater than 0");

        let balance = self.internal_unwrap_balance_of(&token_id, &sender_id);

        let new_balance = balance.checked_sub(amount).expect("not enough balance");
        let mut balances = self
            .balances_per_token
            .get(&token_id)
            .expect("Token not found");
        balances.insert(&sender_id, &new_balance);

        let receiver_balance = self
            .internal_unwrap_balance_of(&token_id, &receiver_id)
            .checked_add(amount)
            .expect("receiver balance overflow");
        balances.insert(&receiver_id, &receiver_balance);
    }

    // Used to get balance of specified account in specified token
    fn internal_unwrap_balance_of(&self, token_id: &TokenId, account_id: &AccountId) -> Balance {
        self.balances_per_token
            .get(token_id)
            .expect("This token does not exist")
            .get(account_id)
            .unwrap_or(0)
    }
}

// TODO: Implement resolver/receiver to test token exchange use-cases.
#[cfg(test)]
mod tests {
    use crate::token::{Token, TokenId};
    use crate::{Balance, MultiTokenContract};
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::{testing_env, AccountId};

    const OWNER_ACCOUNT: usize = 0;

    fn setup_contract() -> (VMContextBuilder, MultiTokenContract) {
        let mut context = VMContextBuilder::new();
        testing_env!(context
            .predecessor_account_id(accounts(OWNER_ACCOUNT))
            .build());
        let contract = MultiTokenContract::new(accounts(OWNER_ACCOUNT));
        (context, contract)
    }

    fn mint_token(context: &mut VMContextBuilder, contract: &mut MultiTokenContract) -> Token {
        testing_env!(context
            .predecessor_account_id(accounts(OWNER_ACCOUNT))
            .build());
        contract.mt_mint(accounts(OWNER_ACCOUNT), u128::MAX.into())
    }

    fn deposit_token(
        context: &mut VMContextBuilder,
        contract: &mut MultiTokenContract,
        account: AccountId,
        token_id: TokenId,
        amount: Balance,
    ) {
        testing_env!(context
            .predecessor_account_id(accounts(OWNER_ACCOUNT))
            .build());
        contract.mt_transfer(account, token_id, amount.into(), None);
    }

    #[test]
    fn list_asset_balances() {
        let (mut ctx, mut contract) = setup_contract();
        let token = mint_token(&mut ctx, &mut contract);
        // mint a second token to fully demonstrate the api.
        _ = mint_token(&mut ctx, &mut contract);

        let user_account = accounts(1);
        let token_amount = 100;

        deposit_token(
            &mut ctx,
            &mut contract,
            user_account.clone(),
            token.token_id,
            token_amount,
        );

        let tokens = contract.mt_tokens(None, None);
        let ids: Vec<_> = tokens
            .clone()
            .into_iter()
            .map(|token| token.token_id)
            .collect();

        let balances = contract.mt_batch_balance_of(user_account.clone(), ids.clone());

        // Verify that all the tokens are listed. Note, that all the contract tokens are listed
        // regardless of whether or not the current user has any.
        assert_eq!(balances.len(), 2);
        // Make sure the user received deposited amount.
        assert_eq!(balances[0], token_amount.into());
        // Make sure that the other token amount is 0, because the user has no balance.
        assert_eq!(balances[1], 0.into());
    }

    // TODO: For the below use-cases we need to implement some sort of a contract, similar to
    // https://github.com/near/near-sdk-rs/blob/d996fc433c4d059fc99ee9ffcdff29870c3e87da/examples/multi-token/test-contract-defi/src/lib.rs#L1-L0.
    // TODO: Add a use-case when one token is exchanged for another, using mt_transfer_call.

    // TODO: Add a use-case when a set of tokens is exchanged for another set of tokens, using
    // mt_batch_transfer_call.
}
