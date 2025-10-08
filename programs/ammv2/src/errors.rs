use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Insufficient balance in user's account for this operation")]
    InsufficientBalance,

    #[msg("Calculated LP token mint amount is zero or negative")]
    InvalidLpTokenAmount,

    #[msg("Attempting to burn more LP tokens than available")]
    ExcessiveBurnAmount,

    #[msg("Output amount is less than the specified minimum")]
    SlippageExceeded,

    #[msg("Invalid fee configuration")]
    InvalidFeeParameters,

    #[msg("Division by zero in calculations")]
    MathOverflow,
}
