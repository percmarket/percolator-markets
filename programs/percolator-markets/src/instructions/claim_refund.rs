use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount};

use crate::errors::PercolatorError;
use crate::state::*;

#[derive(Accounts)]
pub struct ClaimRefund<'info> {
    /// The user claiming their refund.
    #[account(mut)]
    pub user: Signer<'info>,

    /// The cancelled market.
    #[account(
        mut,
        constraint = market.status == MarketStatus::Cancelled @ PercolatorError::InvalidMarketStatus,
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

    /// Market vault.
    /// CHECK: Validated by seeds.
    #[account(
        mut,
        seeds = [b"vault", market.key().as_ref()],
        bump = market.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    /// The user's position token account.
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// YES or NO mint (depending on the position side).
    #[account(mut)]
    pub position_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<ClaimRefund>) -> Result<()> {
    let position = &ctx.accounts.position;
    let refund_amount = position.deposited;

    // Burn the user's position tokens
    let market_key = ctx.accounts.market.key();
    let creator_key = ctx.accounts.market.creator;
    let market_id_bytes = ctx.accounts.market.market_id.to_le_bytes();
    let bump = ctx.accounts.market.bump;
    let seeds: &[&[u8]] = &[
        b"market",
        creator_key.as_ref(),
        market_id_bytes.as_ref(),
        &[bump],
    ];

    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.position_mint.to_account_info(),
                from: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        refund_amount,
    )?;

    // Transfer SOL back from vault to user
    **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -= refund_amount;
    **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += refund_amount;

    // Mark position as settled (refunded)
    let position = &mut ctx.accounts.position;
    position.settled = true;
    position.payout = refund_amount;

    msg!(
        "Refund: {} lamports returned to {} for market #{}",
        refund_amount,
        ctx.accounts.user.key(),
        ctx.accounts.market.market_id,
    );

    Ok(())
}

