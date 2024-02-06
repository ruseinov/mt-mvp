use crate::receiver::MultiTokenReceiver;
use crate::token::{Token, TokenId};
use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, Promise};
use near_sdk::{ext_contract, near_bindgen, require, AccountId, PromiseOrValue};

/// `escrow_transfer` has to be implemented by the MT contract and called within `mt_on_transfer` to
/// facilitate the swap.
/// NOTE: This is just an example interface to demonstrate swap functionality. It's NOT intended to
/// be a part of MT spec.
#[ext_contract(ext_defi_escrow_transfer)]
pub trait EscrowTransfer {
    fn escrow_transfer(
        &mut self,
        receiver_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        change_amounts: Vec<U128>,
    ) -> Promise;
}

#[near_bindgen]
#[derive(BorshSerialize, BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
pub struct DeFi {
    multi_token_account_id: AccountId,
    // this could also contain means of bookkeeping, e.g. standing orders and amounts in escrow.
}

#[near_bindgen]
impl DeFi {
    #[init]
    pub fn new(multi_token_account_id: AccountId) -> Self {
        Self {
            multi_token_account_id,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
enum ExchangeAction {
    Swap {
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
    },

    // NOTE: In real world this should be calculated by the exchange, but the purpose of this
    // exercise is to demonstrate the back-and-forth only.
    SwapWithChange {
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        change_amounts: Vec<U128>,
    },

    // Trigger panic to cover the test case,
    Fail,
}

#[near_bindgen]
impl MultiTokenReceiver for DeFi {
    fn mt_on_transfer(
        &mut self,
        sender_id: AccountId,
        token_ids: Vec<TokenId>,
        amounts: Vec<U128>,
        msg: String,
    ) -> PromiseOrValue<Vec<U128>> {
        // Verify caller.
        require!(
            env::predecessor_account_id() == self.multi_token_account_id,
            "Invalid caller"
        );

        let action: ExchangeAction = near_sdk::serde_json::from_str(&msg).expect("invalid message");

        match action {
            ExchangeAction::Swap { amounts, token_ids } => {
                ext_defi_escrow_transfer::ext(self.multi_token_account_id.clone())
                    .escrow_transfer(
                        sender_id,
                        token_ids,
                        amounts.clone(),
                        vec![0.into(); amounts.len()],
                    )
                    .into()
            }

            ExchangeAction::SwapWithChange {
                token_ids,
                amounts,
                change_amounts,
            } => {
                require!(
                    amounts.len() == change_amounts.len(),
                    "invalid change amounts supplied"
                );

                ext_defi_escrow_transfer::ext(self.multi_token_account_id.clone())
                    .escrow_transfer(sender_id, token_ids, amounts, change_amounts)
                    .into()
            }
            // Just a random failure error, abort.
            ExchangeAction::Fail => env::panic_str("on_transfer error"),
        }
    }
}
