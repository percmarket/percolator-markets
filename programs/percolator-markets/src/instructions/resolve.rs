use anchor_lang::prelude::*;

use crate::errors::PercolatorError;
use crate::state::*;

#[derive(Accounts)]
pub struct ResolveMarket<'info> {
    /// Oracle authority — the only account authorized to resolve.
    #[account(
        constraint = oracle.key() == market.oracle @ PercolatorError::UnauthorizedOracle,
    )]
    pub oracle: Signer<'info>,

    /// The market to resolve.
    #[account(
        mut,
        constraint = market.status == MarketStatus::Open || market.status == MarketStatus::Closed
            @ PercolatorError::AlreadyResolved,
    )]
    pub market: Account<'info, Market>,

    /// Market vault — read balance for h-ratio computation.
    /// CHECK: Validated by seeds.
    #[account(
        seeds = [b"vault", market.key().as_ref()],
        bump = market.vault_bump,
    )]
    pub vault: SystemAccount<'info>,
}

pub fn handler(ctx: Context<ResolveMarket>, outcome: Outcome) -> Result<()> {
    require!(
        outcome != Outcome::Unresolved,
        PercolatorError::InvalidOutcome
    );

    let clock = Clock::get()?;
    let market = &mut ctx.accounts.market;

    // Market must have reached deadline (or we allow early resolution by oracle)
    // For flexibility, we allow oracle to resolve at any time — they are trusted.

    // Compute h-ratio based on current vault balance
    //
    //   h = min(vault_balance, total_claims) / total_claims
    //
    // This is the core Percolator invariant: if the vault can cover all claims,
    // h = 100%. Otherwise, profits are proportionally reduced.
    let vault_balance = ctx.accounts.vault.lamports();
    market.outcome = outcome;
    market.h_ratio_bps = market.compute_h_ratio(vault_balance);
    market.status = MarketStatus::Resolved;

    msg!(
        "Market #{} resolved: outcome={:?}, h_ratio={}bps, vault={}, yes_pool={}, no_pool={}",
        market.market_id,
        outcome as u8,
        market.h_ratio_bps,
        vault_balance,
        market.yes_pool,
        market.no_pool,
    );

    Ok(())
}

