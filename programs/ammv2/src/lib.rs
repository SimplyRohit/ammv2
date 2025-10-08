use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod amm_v2 {
    use super::*;

    /// Initialize a new liquidity pool with two tokens
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        fee_basis_points_numerator: u64,
        fee_basis_points_denominator: u64,
    ) -> Result<()> {
        instructions::initialize_pool::process(
            ctx,
            fee_basis_points_numerator,
            fee_basis_points_denominator,
        )
    }

    /// Add liquidity to the pool and receive LP tokens
    pub fn deposit_liquidity(
        ctx: Context<ManageLiquidity>,
        token_a_amount: u64,
        token_b_amount: u64,
    ) -> Result<()> {
        instructions::manage_liquidity::deposit(ctx, token_a_amount, token_b_amount)
    }

    /// Remove liquidity from the pool by burning LP tokens
    pub fn withdraw_liquidity(ctx: Context<ManageLiquidity>, lp_tokens_to_burn: u64) -> Result<()> {
        instructions::manage_liquidity::withdraw(ctx, lp_tokens_to_burn)
    }

    /// Swap tokens using the constant product formula
    pub fn swap_tokens(
        ctx: Context<SwapTokens>,
        input_amount: u64,
        minimum_output_amount: u64,
    ) -> Result<()> {
        instructions::swap::process(ctx, input_amount, minimum_output_amount)
    }
}
