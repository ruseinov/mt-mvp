use crate::defi::EscrowTransfer;
use crate::token::TokenId;
use crate::MultiTokenContract;
use near_sdk::json_types::U128;
use near_sdk::{env, AccountId, Promise, PromiseOrValue};

impl EscrowTransfer for MultiTokenContract {
    fn escrow_transfer(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        change_amounts: Vec<U128>,
    ) -> Promise {
        // no need to check predecessor acc id, because we are using their escrow account to
        // disburse the funds.
        self.mt_batch_transfer(receiver_id, token_ids, amounts, None);
        PromiseOrValue::Value(change_amounts).into()
    }
}
