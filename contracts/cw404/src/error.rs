use cosmwasm_std::StdError;
use cw_ownable::OwnershipError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Ownership(#[from] OwnershipError),

    #[error(transparent)]
    Version(#[from] cw2::VersionError),

    #[error("token_id already claimed")]
    Claimed {},
   
    #[error("Unauthorized")]
    Unauthorized {},
    
    #[error("InvalidSender")]
    InvalidSender {},
    
    #[error("InvalidRecipient")]
    InvalidRecipient {},
    
    #[error("AlreadyExists")]
    AlreadyExists {},

    #[error("PreventBurn")]
    PreventBurn {},

    #[error("Cannot set approval that is already expired")]
    Expired {},

    #[error("Approval not found for: {spender}")]
    ApprovalNotFound { spender: String },

    #[error("No withdraw address set")]
    NoWithdrawAddress {},
}
