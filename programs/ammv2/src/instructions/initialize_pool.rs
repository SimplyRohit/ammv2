use crate::errors::AmmError;
use crate::state::LiquidityPool;
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

/// Initialize a new AMM liquidity pool
pub fn process(
    ctx: Context<InitializePool>,
    fee_numerator: u64,
    fee_denominator: u64,
) -> Result<()> {
    require!(fee_denominator > 0, AmmError::InvalidFeeParameters);
    require!(
        fee_numerator < fee_denominator,
        AmmError::InvalidFeeParameters
    );

    let pool = &mut ctx.accounts.liquidity_pool;
    pool.fee_numerator = fee_numerator;
    pool.fee_denominator = fee_denominator;
    pool.total_lp_tokens_issued = 0;

    msg!(
        "Pool initialized with fee: {}/{}",
        fee_numerator,
        fee_denominator
    );
    Ok(())
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    /// First token mint in the trading pair
    pub token_a_mint: InterfaceAccount<'info, Mint>,

    /// Second token mint in the trading pair
    pub token_b_mint: InterfaceAccount<'info, Mint>,

    /// PDA account that stores pool configuration and state
    #[account(
        init,
        space = 8 + LiquidityPool::ACCOUNT_SIZE,
        payer = payer,
        seeds = [
            b"liquidity_pool",
            token_a_mint.key().as_ref(),
            token_b_mint.key().as_ref()
        ],
        bump,
    )]
    pub liquidity_pool: Box<Account<'info, LiquidityPool>>,

    /// PDA authority that controls the pool's vaults and LP token minting
    #[account(
        seeds = [b"pool_authority", liquidity_pool.key().as_ref()],
        bump
    )]
    pub pool_authority: SystemAccount<'info>,

    /// Vault to hold token A reserves
    #[account(
        init,
        payer = payer,
        seeds = [b"token_a_vault", liquidity_pool.key().as_ref()],
        bump,
        token::mint = token_a_mint,
        token::authority = pool_authority,
    )]
    pub token_a_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Vault to hold token B reserves
    #[account(
        init,
        payer = payer,
        seeds = [b"token_b_vault", liquidity_pool.key().as_ref()],
        bump,
        token::mint = token_b_mint,
        token::authority = pool_authority,
    )]
    pub token_b_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// LP token mint - represents shares of the liquidity pool
    #[account(
        init,
        payer = payer,
        seeds = [b"lp_token_mint", liquidity_pool.key().as_ref()],
        bump,
        mint::decimals = 9,
        mint::authority = pool_authority,
    )]
    pub lp_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Account that pays for initialization
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Required system programs
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
