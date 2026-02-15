use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("PERCmkT7XHFjnMGiLBKo9Qxmf4BBJY6oSVhLwMpXuq");

#[program]
pub mod percolator_markets {
    use super::*;

    /// Create a new binary prediction market.
    ///
    /// The market vault is funded by an initial seed deposit from the creator.
    /// Resolution criteria are encoded in `rule` + `target_value`.
    pub fn create_market(
        ctx: Context<CreateMarket>,
        params: CreateMarketParams,
    ) -> Result<()> {
        instructions::create_market::handler(ctx, params)
    }

    /// Place a bet on YES or NO.
    ///
    /// Transfers `amount` from the bettor into the market vault and mints
    /// the corresponding position token (YES-mint or NO-mint).
    pub fn place_bet(ctx: Context<PlaceBet>, side: BetSide, amount: u64) -> Result<()> {
        instructions::place_bet::handler(ctx, side, amount)
    }

    /// Resolve the market outcome.
    ///
    /// Only callable by the designated oracle authority.
    /// Sets `outcome` to YES or NO based on the resolution condition.
    pub fn resolve_market(ctx: Context<ResolveMarket>, outcome: Outcome) -> Result<()> {
        instructions::resolve::handler(ctx, outcome)
    }

    /// Settle a user's position after market resolution.
    ///
    /// Computes payout using the Percolator two-claim model:
    ///   - Capital (senior): returned first from vault
    ///   - Profit  (junior): share of loser pool × h-ratio
    ///
    /// h = min(vault_balance, total_winning_claims) / total_winning_claims
    ///
    /// If h < 1, profits are haircut proportionally — the market NEVER
    /// becomes insolvent.
    pub fn settle(ctx: Context<Settle>) -> Result<()> {
        instructions::settle::handler(ctx)
    }

    /// Cancel a market before resolution (creator or authority only).
    ///
    /// All bettors can claim full refund via `claim_refund`.
    pub fn cancel_market(ctx: Context<CancelMarket>) -> Result<()> {
        instructions::cancel::handler(ctx)
    }

    /// Claim refund from a cancelled market.
    ///
    /// Burns the user's position tokens and returns the equivalent SOL.
    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        instructions::claim_refund::handler(ctx)
    }
}

