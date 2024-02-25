use anchor_lang::prelude::*;

#[error_code]
pub enum PasswordEscrowError {
    ZeroFeeBps,
    ZeroAmount,
}
