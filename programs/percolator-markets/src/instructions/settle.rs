use anchor_lang::prelude::*;

use crate::errors::PercolatorError;
use crate::state::*;

#[derive(Accounts)]
pub struct Settle<'info> {
    /// The user claiming their payout.
    #[account(mut)]
    pub user: Signer<'info>,

    /// The resolved market.
    #[account(
        mut,
        constraint = market.status == MarketStatus::Resolved @ PercolatorError::InvalidMarketStatus,
    )]
    pub market: Account<'info, Market>,

    /// User position PDA.
    #[account(
        mut,
        seeds = [b"position", market.key().as_ref(), user.key().as_ref()],
        bump = position.bump,
        constraint = !position.settled @ PercolatorError::AlreadySettled,
        constraint = position.user == user.key() @ PercolatorError::NoPosition,
    )]
    pub position: Account<'info, UserPosition>,

    /// Market vault — source of payout funds.
    /// CHECK: Validated by seeds.
    #[account(
        mut,
        seeds = [b"vault", market.key().as_ref()],
        bump = market.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Settle>) -> Result<()> {
    let market = &ctx.accounts.market;
    let position = &ctx.accounts.position;

    // Determine if the user is on the winning side
    let is_winner = match (market.outcome, position.side) {
        (Outcome::Yes, BetSide::Yes) => true,
        (Outcome::No, BetSide::No) => true,
        _ => false,
    };

    require!(is_winner, PercolatorError::LosingSide);

    // ────────────────────────────────────────────────────────────
    // Percolator Two-Claim Settlement
    // ────────────────────────────────────────────────────────────
    //
    // The payout consists of two components:
    //
    //   1. Capital (Senior Claim)
    //      The user's original stake. This is returned FIRST — it has
    //      priority over profit claims. In Percolator terms, this is
    //      the "withdrawable principal".
    //
    //   2. Profit (Junior Claim)
    //      The user's proportional share of the losing pool. This is
    //      subject to the h-ratio haircut:
    //
    //        profit_share = (user_stake / winner_pool) × loser_pool
    //        actual_profit = profit_share × h
    //
    //      Where h = h_ratio_bps / 10000.
    //
    // The h-ratio ensures that if the vault cannot cover all claims
    // (which should not happen in normal operation), profits are
    // reduced proportionally rather than some users getting paid and
    // others not. This is the core insolvency-safety guarantee from
    // the Percolator risk engine.
    //
    // Invariant: settled_amount <= vault_balance (always)
    // ────────────────────────────────────────────────────────────

    let payout = market.calculate_payout(position.deposited);

    // Safety check: ensure vault has enough
    let vault_balance = ctx.accounts.vault.lamports();
    require!(payout <= vault_balance, PercolatorError::VaultInsolvency);

    // Transfer payout from vault PDA to user
    let market_key = ctx.accounts.market.key();
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        market_key.as_ref(),
        &[market.vault_bump],
    ];

    // Direct lamport transfer from PDA
    **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -= payout;
    **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += payout;

    // Update position
    let position = &mut ctx.accounts.position;
    position.settled = true;
    position.payout = payout;

    // Update market settlement tracking
    let market = &mut ctx.accounts.market;
    market.settled_amount = market.settled_amount.checked_add(payout)
        .ok_or(PercolatorError::Overflow)?;
    market.settlements_count = market.settlements_count.checked_add(1)
        .ok_or(PercolatorError::Overflow)?;

    msg!(
        "Settled: user={} payout={} (capital={} + profit×h), market #{}",
        ctx.accounts.user.key(),
        payout,
        position.deposited,
        market.market_id,
    );

    Ok(())
}

