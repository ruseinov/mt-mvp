use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::AccountId;

pub type TokenId = String;

/// Info on individual token
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "abi", derive(schemars::JsonSchema))]
#[serde(crate = "near_sdk::serde")]
pub struct Token {
    pub token_id: String,
    // Question: what do we need this for? Logically the owner of the token is somebody who has the
    // control of it's supply when it's minted. Once those tokens start being transferred to other
    // accounts - this field is basically irrelevant.
    // If we want to keep track of the original owner - that could be done via events/metadata.
    pub owner_id: AccountId,
    /// Total amount generated
    pub supply: u128,
}
