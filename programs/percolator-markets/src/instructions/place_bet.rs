use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::errors::PercolatorError;
use crate::state::*;

#[derive(Accounts)]
pub struct PlaceBet<'info> {
    /// The bettor placing the wager.
    #[account(mut)]
    pub bettor: Signer<'info>,

    /// The prediction market.
    #[account(
        mut,
        constraint = market.status == MarketStatus::Open @ PercolatorError::InvalidMarketStatus,
    )]
    pub market: Account<'info, Market>,

    /// User position PDA — created on first bet, updated on subsequent bets.
    #[account(
        init_if_needed,
        payer = bettor,
        space = UserPosition::SIZE,
        seeds = [b"position", market.key().as_ref(), bettor.key().as_ref()],
        bump,
    )]
    pub position: Account<'info, UserPosition>,

    /// Market vault — receives the SOL deposit.
    /// CHECK: Validated by seeds constraint.
    #[account(
        mut,
        seeds = [b"vault", market.key().as_ref()],
        bump = market.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    /// YES token mint (market is authority).
    #[account(
        mut,
        seeds = [b"yes_mint", market.key().as_ref()],
        bump,
    )]
    pub yes_mint: Account<'info, Mint>,

    /// NO token mint (market is authority).
    #[account(
        mut,
        seeds = [b"no_mint", market.key().as_ref()],
        bump,
    )]
    pub no_mint: Account<'info, Mint>,

    /// Bettor's token account for the chosen side.
    #[account(mut)]
    pub bettor_token_account: Account<'info, TokenAccount>,

    /// Global config for volume tracking.
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<PlaceBet>, side: BetSide, amount: u64) -> Result<()> {
    require!(amount > 0, PercolatorError::ZeroBetAmount);

    let clock = Clock::get()?;
    let market = &ctx.accounts.market;
    require!(
        clock.unix_timestamp < market.deadline,
        PercolatorError::MarketExpired
    );

    // Transfer SOL from bettor to vault
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.bettor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        ),
        amount,
    )?;

    // Determine which mint to use
    let mint = match side {
        BetSide::Yes => ctx.accounts.yes_mint.to_account_info(),
        BetSide::No => ctx.accounts.no_mint.to_account_info(),
    };

    // Mint position tokens to bettor
    // Market PDA is the mint authority
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

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint,
                to: ctx.accounts.bettor_token_account.to_account_info(),
                authority: ctx.accounts.market.to_account_info(),
            },
            &[seeds],
        ),
        amount, // 1:1 — each lamport = 1 position token
    )?;

    // Update market pools
    let market = &mut ctx.accounts.market;
    match side {
        BetSide::Yes => {
            market.yes_pool = market.yes_pool.checked_add(amount)
                .ok_or(PercolatorError::Overflow)?;
        }
        BetSide::No => {
            market.no_pool = market.no_pool.checked_add(amount)
                .ok_or(PercolatorError::Overflow)?;
        }
    }

    // Update user position
    let position = &mut ctx.accounts.position;
    if position.deposited == 0 {
        // First bet — initialize
        position.market = market.key();
        position.user = ctx.accounts.bettor.key();
        position.side = side;
        position.bump = ctx.bumps.position;
    }
    position.deposited = position.deposited.checked_add(amount)
        .ok_or(PercolatorError::Overflow)?;

    // Track global volume
    let config = &mut ctx.accounts.config;
    config.total_volume = config.total_volume.checked_add(amount)
        .ok_or(PercolatorError::Overflow)?;

    msg!(
        "Bet placed: {} lamports on {:?} for market #{}",
        amount,
        side as u8,
        market.market_id,
    );

    Ok(())
}

