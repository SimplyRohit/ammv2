use crate::errors::AmmError;
use crate::state::LiquidityPool;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenAccount, TokenInterface};

/// Swap tokens using constant product AMM formula (x * y = k)
pub fn process(
    ctx: Context<SwapTokens>,
    input_amount: u64,
    minimum_output_amount: u64,
) -> Result<()> {
    // Verify user has sufficient input tokens
    require!(
        ctx.accounts.user_input_token_account.amount >= input_amount,
        AmmError::InsufficientBalance
    );

    let pool = &ctx.accounts.liquidity_pool;
    let input_vault_balance = ctx.accounts.input_token_vault.amount as u128;
    let output_vault_balance = ctx.accounts.output_token_vault.amount as u128;
    let input_amount_u128 = input_amount as u128;

    // Calculate trading fee
    let fee_amount = pool.calculate_fee(input_amount_u128)?;
    let input_after_fee = input_amount_u128
        .checked_sub(fee_amount)
        .ok_or(AmmError::MathOverflow)?;

    msg!(
        "Swap details - Input: {}, Fee: {}, Net input: {}",
        input_amount,
        fee_amount,
        input_after_fee
    );

    // Constant product formula: x * y = k
    // Where k is the invariant that must be maintained
    let invariant = input_vault_balance
        .checked_mul(output_vault_balance)
        .ok_or(AmmError::MathOverflow)?;

    // New input vault balance after adding tokens
    let new_input_vault_balance = input_vault_balance
        .checked_add(input_after_fee)
        .ok_or(AmmError::MathOverflow)?;

    // Calculate new output vault balance to maintain invariant
    let new_output_vault_balance = invariant
        .checked_div(new_input_vault_balance)
        .ok_or(AmmError::MathOverflow)?;

    // Output amount = current balance - new balance
    let output_amount = output_vault_balance
        .checked_sub(new_output_vault_balance)
        .ok_or(AmmError::MathOverflow)?;

    msg!("Calculated output amount: {}", output_amount);

    // Slippage protection
    require!(
        output_amount >= minimum_output_amount as u128,
        AmmError::SlippageExceeded
    );

    // Setup PDA signer
    let pool_key = ctx.accounts.liquidity_pool.key();
    let authority_bump = ctx.bumps.pool_authority;
    let authority_seeds = &[b"pool_authority", pool_key.as_ref(), &[authority_bump]];
    let signer_seeds = &[&authority_seeds[..]];

    // Transfer output tokens from vault to user
    token_interface::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.output_token_vault.to_account_info(),
                to: ctx.accounts.user_output_token_account.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        output_amount as u64,
    )?;

    // Transfer input tokens from user to vault (including fee)
    token_interface::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.user_input_token_account.to_account_info(),
                to: ctx.accounts.input_token_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        input_amount,
    )?;

    msg!(
        "Swap completed - Input: {}, Output: {}",
        input_amount,
        output_amount
    );

    Ok(())
}

#[derive(Accounts)]
pub struct SwapTokens<'info> {
    /// Pool state account
    #[account(mut)]
    pub liquidity_pool: Box<Account<'info, LiquidityPool>>,

    /// Pool authority PDA
    #[account(
        seeds = [b"pool_authority", liquidity_pool.key().as_ref()],
        bump
    )]
    pub pool_authority: SystemAccount<'info>,

    /// Vault for input token (token being sold)
    #[account(
        mut,
        constraint = input_token_vault.owner == pool_authority.key(),
        constraint = input_token_vault.mint == user_input_token_account.mint,
    )]
    pub input_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault for output token (token being bought)
    #[account(
        mut,
        constraint = output_token_vault.owner == pool_authority.key(),
        constraint = output_token_vault.mint == user_output_token_account.mint,
    )]
    pub output_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's input token account (source)
    #[account(mut)]
    pub user_input_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's output token account (destination)
    #[account(mut)]
    pub user_output_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User authority
    pub user: Signer<'info>,

    /// Token program
    pub token_program: Interface<'info, TokenInterface>,
}

