use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

use crate::errors::PercolatorError;
use crate::state::*;

/// Parameters for creating a new prediction market.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateMarketParams {
    /// Human-readable prediction question (max 256 bytes).
    pub question: String,

    /// Resolution rule.
    pub rule: MarketRule,

    /// Target value for resolution (interpretation depends on `rule`).
    pub target_value: u64,

    /// Token mint address that this market is about.
    pub token_mint: Pubkey,

    /// Oracle authority pubkey that can resolve this market.
    pub oracle: Pubkey,

    /// Unix timestamp deadline.
    pub deadline: i64,
}

#[derive(Accounts)]
#[instruction(params: CreateMarketParams)]
pub struct CreateMarket<'info> {
    /// Market creator — pays for account allocation.
    #[account(mut)]
    pub creator: Signer<'info>,

    /// Global config — provides next_market_id.
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, GlobalConfig>,

    /// Market PDA — the core account for this prediction market.
    #[account(
        init,
        payer = creator,
        space = Market::SIZE,
        seeds = [
            b"market",
            creator.key().as_ref(),
            config.next_market_id.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub market: Account<'info, Market>,

    /// Vault PDA — holds all SOL deposits for this market.
    /// CHECK: Initialized as a PDA; no data, just lamports.
    #[account(
        mut,
        seeds = [b"vault", market.key().as_ref()],
        bump,
    )]
    pub vault: SystemAccount<'info>,

    /// YES position token mint.
    #[account(
        init,
        payer = creator,
        mint::decimals = 0,
        mint::authority = market,
        seeds = [b"yes_mint", market.key().as_ref()],
        bump,
    )]
    pub yes_mint: Account<'info, Mint>,

    /// NO position token mint.
    #[account(
        init,
        payer = creator,
        mint::decimals = 0,
        mint::authority = market,
        seeds = [b"no_mint", market.key().as_ref()],
        bump,
    )]
    pub no_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<CreateMarket>, params: CreateMarketParams) -> Result<()> {
    // Validate inputs
    require!(
        params.question.len() <= 256,
        PercolatorError::QuestionTooLong
    );

    let clock = Clock::get()?;
    require!(
        params.deadline > clock.unix_timestamp,
        PercolatorError::DeadlineInPast
    );

    // Populate market account
    let market = &mut ctx.accounts.market;
    let config = &mut ctx.accounts.config;

    market.market_id = config.next_market_id;
    market.creator = ctx.accounts.creator.key();
    market.oracle = params.oracle;
    market.question = params.question;
    market.rule = params.rule;
    market.target_value = params.target_value;
    market.token_mint = params.token_mint;
    market.deadline = params.deadline;
    market.status = MarketStatus::Open;
    market.outcome = Outcome::Unresolved;
    market.yes_pool = 0;
    market.no_pool = 0;
    market.yes_mint = ctx.accounts.yes_mint.key();
    market.no_mint = ctx.accounts.no_mint.key();
    market.vault = ctx.accounts.vault.key();
    market.vault_bump = ctx.bumps.vault;
    market.bump = ctx.bumps.market;
    market.h_ratio_bps = 10_000; // 100% until resolution
    market.settled_amount = 0;
    market.settlements_count = 0;

    // Increment global counter
    config.next_market_id = config.next_market_id.checked_add(1).unwrap();
    config.total_markets = config.total_markets.checked_add(1).unwrap();

    msg!(
        "Market #{} created: {} | deadline: {} | rule: {:?}",
        market.market_id,
        market.question,
        market.deadline,
        market.rule as u8,
    );

    Ok(())
}

