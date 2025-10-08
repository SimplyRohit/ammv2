use anchor_lang::prelude::*;

/// Stores the state of a liquidity pool
#[account]
#[derive(Default)]
pub struct LiquidityPool {
    /// Total LP tokens minted to all liquidity providers
    pub total_lp_tokens_issued: u64,

    /// Numerator for fee calculation (e.g., 3 for 0.3% with denominator 1000)
    pub fee_numerator: u64,

    /// Denominator for fee calculation (e.g., 1000 for 0.3% fee)
    pub fee_denominator: u64,
}

impl LiquidityPool {
    /// Size calculation for account allocation
    /// 8 bytes discriminator + 8 + 8 + 8 for the fields
    pub const ACCOUNT_SIZE: usize = 8 + 8 + 8 + 8;

    /// Calculate fee amount from input
    pub fn calculate_fee(&self, amount: u128) -> Result<u128> {
        amount
            .checked_mul(self.fee_numerator as u128)
            .and_then(|v| v.checked_div(self.fee_denominator as u128))
            .ok_or(error!(crate::errors::AmmError::MathOverflow))
    }
}
