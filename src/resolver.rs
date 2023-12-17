use crate::token::TokenId;
use near_sdk::ext_contract;
use near_sdk::json_types::U128;
use near_sdk::AccountId;

/// `resolve_transfer` must be called after `on_transfer`
#[ext_contract(ext_mt_resolver)]
pub trait MultiTokenResolver {
    /// Finalizes chain of cross-contract calls that started from `mt_{batch_}transfer_call`
    ///
    /// Flow:
    ///
    /// 1. Sender calls `mt_transfer_call` on MT contract.
    /// 2. MT contract transfers tokens from sender to receiver.
    /// 3. MT contract calls `on_transfer` on receiver contract.
    /// 4+. [receiver may make cross-contract calls].
    /// N. MT contract resolves the chain with `mt_resolve_transfer` with some final actions.
    ///
    /// Requirements:
    /// * Must only be callable by the contract itself.
    /// * If the promise chain failed, contract MUST revert tokens transfer.
    /// * If the promise chain resolves with `true`, contract MUST return tokens to the sender.
    ///
    /// Arguments:
    /// * `sender_id` the sender of `transfer_call`.
    /// * `receiver_id` the `receiver_id` argument given to `transfer_call`.
    /// * `token_ids` the vector of `token_id` argument given to `transfer_call`.
    ///
    /// Returns total amount spent by the `receiver_id`, corresponding to the `token_ids`.
    ///
    /// Example: if sender calls `transfer_call({ "amounts": ["100"], token_ids: ["55"], receiver_id: "games" })`,
    /// but `receiver_id` only uses 80, `on_transfer` will resolve with `["20"]`, and `resolve_transfer`
    /// will return `[80]`.

    fn mt_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
    ) -> Vec<U128>;
}
