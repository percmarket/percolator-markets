use anchor_lang::prelude::*;

use crate::errors::PercolatorError;
use crate::state::*;

#[derive(Accounts)]
pub struct CancelMarket<'info> {
    /// Market creator or oracle authority.
    #[account(
        constraint = authority.key() == market.creator || authority.key() == market.oracle
            @ PercolatorError::UnauthorizedCreator,
    )]
    pub authority: Signer<'info>,

    /// The market to cancel.
    #[account(
        mut,
        constraint = market.status == MarketStatus::Open || market.status == MarketStatus::Closed
            @ PercolatorError::CannotCancelResolved,
    )]
    pub market: Account<'info, Market>,
}

pub fn handler(ctx: Context<CancelMarket>) -> Result<()> {
    let market = &mut ctx.accounts.market;
    market.status = MarketStatus::Cancelled;

    msg!(
        "Market #{} cancelled by {}",
        market.market_id,
        ctx.accounts.authority.key(),
    );

    Ok(())
}

