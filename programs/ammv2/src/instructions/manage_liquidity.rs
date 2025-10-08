use crate::errors::AmmError;
use crate::state::LiquidityPool;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Burn, Mint, MintTo, TokenAccount, TokenInterface, Transfer,
};

/// Add liquidity to the pool
pub fn deposit(
    ctx: Context<ManageLiquidity>,
    token_a_amount: u64,
    token_b_amount: u64,
) -> Result<()> {
    // Verify user has sufficient balance
    require!(
        ctx.accounts.user_token_a_account.amount >= token_a_amount,
        AmmError::InsufficientBalance
    );
    require!(
        ctx.accounts.user_token_b_account.amount >= token_b_amount,
        AmmError::InsufficientBalance
    );

    let vault_a_balance = ctx.accounts.token_a_vault.amount;
    let vault_b_balance = ctx.accounts.token_b_vault.amount;
    let pool = &mut ctx.accounts.liquidity_pool;

    let actual_token_a_deposit = token_a_amount;
    let actual_token_b_deposit: u64;
    let lp_tokens_to_mint: u64;

    msg!(
        "Current vault balances - Token A: {}, Token B: {}",
        vault_a_balance,
        vault_b_balance
    );

    // Initial liquidity deposit (pool is empty)
    if vault_a_balance == 0 && vault_b_balance == 0 {
        msg!(
            "Initial deposit - Token A: {}, Token B: {}",
            token_a_amount,
            token_b_amount
        );

        // For first deposit, LP tokens = geometric mean of deposits (divided by 2 via bit shift)
        lp_tokens_to_mint = (token_a_amount + token_b_amount) >> 1;
        actual_token_b_deposit = token_b_amount;
    } else {
        // Subsequent deposits must maintain pool ratio
        // Calculate required token B based on token A deposit and current pool ratio
        let exchange_rate_b_per_a = (vault_b_balance as u128)
            .checked_div(vault_a_balance as u128)
            .ok_or(AmmError::MathOverflow)?;

        let required_token_b = (token_a_amount as u128)
            .checked_mul(exchange_rate_b_per_a)
            .ok_or(AmmError::MathOverflow)? as u64;

        msg!(
            "Exchange rate (B/A): {}, Required Token B: {}",
            exchange_rate_b_per_a,
            required_token_b
        );

        require!(
            required_token_b <= token_b_amount,
            AmmError::InsufficientBalance
        );

        actual_token_b_deposit = required_token_b;

        // LP tokens minted proportional to share of pool
        // LP_mint = (deposit_B * total_LP) / vault_B_balance
        lp_tokens_to_mint = (actual_token_b_deposit as u128)
            .checked_mul(pool.total_lp_tokens_issued as u128)
            .and_then(|v| v.checked_div(vault_b_balance as u128))
            .ok_or(AmmError::MathOverflow)? as u64;

        msg!("LP tokens to mint: {}", lp_tokens_to_mint);
    }

    require!(lp_tokens_to_mint > 0, AmmError::InvalidLpTokenAmount);

    // Update pool state
    pool.total_lp_tokens_issued = pool
        .total_lp_tokens_issued
        .checked_add(lp_tokens_to_mint)
        .ok_or(AmmError::MathOverflow)?;

    // Mint LP tokens to user
    let pool_key = ctx.accounts.liquidity_pool.key();
    let authority_bump = ctx.bumps.pool_authority;
    let authority_seeds = &[b"pool_authority", pool_key.as_ref(), &[authority_bump]];
    let signer_seeds = &[&authority_seeds[..]];

    token_interface::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.lp_token_mint.to_account_info(),
                to: ctx.accounts.user_lp_token_account.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        lp_tokens_to_mint,
    )?;

    // Transfer token A from user to vault
    token_interface::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.user_token_a_account.to_account_info(),
                to: ctx.accounts.token_a_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        actual_token_a_deposit,
    )?;

    // Transfer token B from user to vault
    token_interface::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.user_token_b_account.to_account_info(),
                to: ctx.accounts.token_b_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        actual_token_b_deposit,
    )?;

    msg!(
        "Liquidity added - Token A: {}, Token B: {}, LP tokens: {}",
        actual_token_a_deposit,
        actual_token_b_deposit,
        lp_tokens_to_mint
    );

    Ok(())
}

/// Remove liquidity from the pool
pub fn withdraw(ctx: Context<ManageLiquidity>, lp_tokens_to_burn: u64) -> Result<()> {
    // Verify user has sufficient LP tokens
    require!(
        ctx.accounts.user_lp_token_account.amount >= lp_tokens_to_burn,
        AmmError::InsufficientBalance
    );

    let pool_key = ctx.accounts.liquidity_pool.key();
    let pool = &mut ctx.accounts.liquidity_pool;
    require!(
        pool.total_lp_tokens_issued >= lp_tokens_to_burn,
        AmmError::ExcessiveBurnAmount
    );

    let vault_a_balance = ctx.accounts.token_a_vault.amount as u128;
    let vault_b_balance = ctx.accounts.token_b_vault.amount as u128;
    let burn_amount = lp_tokens_to_burn as u128;
    let total_lp_supply = pool.total_lp_tokens_issued as u128;

    // Calculate proportional withdrawal amounts
    // withdrawn_A = (LP_burned * vault_A) / total_LP
    let token_a_withdrawal = burn_amount
        .checked_mul(vault_a_balance)
        .and_then(|v| v.checked_div(total_lp_supply))
        .ok_or(AmmError::MathOverflow)? as u64;

    let token_b_withdrawal = burn_amount
        .checked_mul(vault_b_balance)
        .and_then(|v| v.checked_div(total_lp_supply))
        .ok_or(AmmError::MathOverflow)? as u64;

    // Setup PDA signer

    let authority_bump = ctx.bumps.pool_authority;
    let authority_seeds = &[b"pool_authority", pool_key.as_ref(), &[authority_bump]];
    let signer_seeds = &[&authority_seeds[..]];

    // Transfer token A from vault to user
    token_interface::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.token_a_vault.to_account_info(),
                to: ctx.accounts.user_token_a_account.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        token_a_withdrawal,
    )?;

    // Transfer token B from vault to user
    token_interface::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::Transfer {
                from: ctx.accounts.token_b_vault.to_account_info(),
                to: ctx.accounts.user_token_b_account.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        token_b_withdrawal,
    )?;

    // Burn LP tokens
    token_interface::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.lp_token_mint.to_account_info(),
                from: ctx.accounts.user_lp_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        lp_tokens_to_burn,
    )?;

    // Update pool state
    pool.total_lp_tokens_issued = pool
        .total_lp_tokens_issued
        .checked_sub(lp_tokens_to_burn)
        .ok_or(AmmError::MathOverflow)?;

    msg!(
        "Liquidity removed - Token A: {}, Token B: {}, LP tokens burned: {}",
        token_a_withdrawal,
        token_b_withdrawal,
        lp_tokens_to_burn
    );

    Ok(())
}

#[derive(Accounts)]
pub struct ManageLiquidity<'info> {
    /// Pool state account
    #[account(mut)]
    pub liquidity_pool: Box<Account<'info, LiquidityPool>>,

    /// Pool authority PDA
    #[account(
        seeds = [b"pool_authority", liquidity_pool.key().as_ref()],
        bump
    )]
    pub pool_authority: SystemAccount<'info>,

    /// Token A vault - must match user's token A mint
    #[account(
        mut,
        constraint = token_a_vault.mint == user_token_a_account.mint,
        seeds = [b"token_a_vault", liquidity_pool.key().as_ref()],
        bump
    )]
    pub token_a_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Token B vault - must match user's token B mint
    #[account(
        mut,
        constraint = token_b_vault.mint == user_token_b_account.mint,
        seeds = [b"token_b_vault", liquidity_pool.key().as_ref()],
        bump
    )]
    pub token_b_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// LP token mint
    #[account(
        mut,
        constraint = lp_token_mint.key() == user_lp_token_account.mint,
        seeds = [b"lp_token_mint", liquidity_pool.key().as_ref()],
        bump
    )]
    pub lp_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// User's token A account
    #[account(mut)]
    pub user_token_a_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's token B account
    #[account(mut)]
    pub user_token_b_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's LP token account
    #[account(mut)]
    pub user_lp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User authority
    pub user: Signer<'info>,

    /// Token program
    pub token_program: Interface<'info, TokenInterface>,
}
