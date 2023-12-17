use crate::token::TokenId;
use near_sdk::json_types::U128;
use near_sdk::{ext_contract, AccountId, PromiseOrValue};
/// Used when an MT is transferred using `transfer_call`. This trait should be implemented on receiving contract
#[ext_contract(ext_mt_receiver)]
pub trait MultiTokenReceiver {
    /// Execute an action upon token receipt.
    ///
    /// ## Requirements:
    /// * Callers must be explicitly whitelisted.
    /// * `token_ids` length must match that of `amounts`.
    ///
    /// ## Arguments:
    /// * `sender_id` the sender of `transfer_call`.
    /// * `token_ids` the `token_ids` argument given to `transfer_call`.
    /// * `amounts` the `amounts` argument given to `transfer_call`
    /// * `msg` information necessary for this contract to know how to process the request. This may
    /// include method names and/or arguments.
    ///
    /// Returns the number of unused tokens.
    fn mt_on_transfer(
        &mut self,
        sender_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>>;
}
