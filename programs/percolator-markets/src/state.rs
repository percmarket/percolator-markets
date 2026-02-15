use anchor_lang::prelude::*;

/// ─── Market Account ───────────────────────────────────────────────
///
/// PDA: seeds = [b"market", creator.key, market_id.to_le_bytes()]
///
/// Stores all state for a single binary prediction market.
#[account]
#[derive(Default)]
pub struct Market {
    /// Unique numeric identifier (incrementing).
    pub market_id: u64,

    /// Creator's public key.
    pub creator: Pubkey,

    /// Oracle authority that can resolve the market.
    pub oracle: Pubkey,

    /// Human-readable question (max 256 bytes).
    pub question: String,

    /// Resolution rule type.
    pub rule: MarketRule,

    /// Target value for resolution (e.g. market cap threshold in USD × 10^6).
    pub target_value: u64,

    /// Token mint address this market is about.
    pub token_mint: Pubkey,

    /// Market deadline (Unix timestamp). After this, resolution can be called.
    pub deadline: i64,

    /// Current market status.
    pub status: MarketStatus,

    /// Resolved outcome (only valid when status == Resolved).
    pub outcome: Outcome,

    // ─── Pool accounting ───
    /// Total lamports deposited into YES side.
    pub yes_pool: u64,

    /// Total lamports deposited into NO side.
    pub no_pool: u64,

    /// YES position token mint.
    pub yes_mint: Pubkey,

    /// NO position token mint.
    pub no_mint: Pubkey,

    /// Market vault PDA (holds all SOL).
    pub vault: Pubkey,

    /// Vault bump seed.
    pub vault_bump: u8,

    /// Market PDA bump seed.
    pub bump: u8,

    // ─── Settlement state ───
    /// Computed h-ratio (basis points, 0–10000). Set at resolution time.
    pub h_ratio_bps: u16,

    /// Total lamports already paid out during settlement.
    pub settled_amount: u64,

    /// Number of individual settlements completed.
    pub settlements_count: u64,

    /// Reserved space for future upgrades.
    pub _reserved: [u8; 128],
}

impl Market {
    /// Account size for Anchor allocation.
    pub const SIZE: usize = 8  // discriminator
        + 8                     // market_id
        + 32                    // creator
        + 32                    // oracle
        + (4 + 256)             // question (String: 4-byte len + max 256 chars)
        + 1                     // rule
        + 8                     // target_value
        + 32                    // token_mint
        + 8                     // deadline
        + 1                     // status
        + 1                     // outcome
        + 8                     // yes_pool
        + 8                     // no_pool
        + 32                    // yes_mint
        + 32                    // no_mint
        + 32                    // vault
        + 1                     // vault_bump
        + 1                     // bump
        + 2                     // h_ratio_bps
        + 8                     // settled_amount
        + 8                     // settlements_count
        + 128;                  // reserved

    /// Compute h-ratio at resolution time.
    ///
    /// h = min(vault_balance, total_winning_claims) / total_winning_claims
    ///
    /// Returns basis points (0–10000).
    ///
    /// # Invariant
    /// h ≤ 1.0 always. If the vault holds enough to pay all winners,
    /// h = 10000 (100%). Otherwise, profits are haircut proportionally.
    pub fn compute_h_ratio(&self, vault_balance: u64) -> u16 {
        let winner_pool = match self.outcome {
            Outcome::Yes => self.yes_pool,
            Outcome::No => self.no_pool,
            Outcome::Unresolved => return 10_000,
        };

        let loser_pool = match self.outcome {
            Outcome::Yes => self.no_pool,
            Outcome::No => self.yes_pool,
            Outcome::Unresolved => return 10_000,
        };

        if winner_pool == 0 {
            return 10_000;
        }

        // Total claims = winner stakes + loser pool (the profit to distribute)
        let total_claims = winner_pool.saturating_add(loser_pool);

        if vault_balance >= total_claims {
            10_000 // fully solvent
        } else {
            // h = vault / total_claims, scaled to basis points
            ((vault_balance as u128 * 10_000) / total_claims as u128) as u16
        }
    }

    /// Calculate payout for a winning position.
    ///
    /// payout = capital + profit × h
    ///
    /// Where:
    ///   capital = user_stake (senior claim, returned first)
    ///   profit  = (user_stake / winner_pool) × loser_pool (junior claim)
    ///   h       = h_ratio_bps / 10000
    pub fn calculate_payout(&self, user_stake: u64) -> u64 {
        let winner_pool = match self.outcome {
            Outcome::Yes => self.yes_pool,
            Outcome::No => self.no_pool,
            Outcome::Unresolved => return 0,
        };

        let loser_pool = match self.outcome {
            Outcome::Yes => self.no_pool,
            Outcome::No => self.yes_pool,
            Outcome::Unresolved => return 0,
        };

        if winner_pool == 0 {
            return 0;
        }

        // Capital: senior claim (returned in full up to vault capacity)
        let capital = user_stake;

        // Profit: junior claim = proportional share of loser pool
        let profit = (user_stake as u128)
            .checked_mul(loser_pool as u128)
            .unwrap_or(0)
            / winner_pool as u128;

        // Apply h-ratio haircut to profit
        let profit_after_h = (profit * self.h_ratio_bps as u128) / 10_000;

        capital.saturating_add(profit_after_h as u64)
    }
}

/// ─── Market Rule ──────────────────────────────────────────────────
///
/// Determines how the market is resolved.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketRule {
    /// Token reaches a target market cap (in USD × 10^6).
    #[default]
    MarketCapTarget,

    /// Token reaches a target price (in USD × 10^9, 9 decimal places).
    PriceTarget,

    /// Token maintains a minimum market cap throughout the period.
    MarketCapFloor,

    /// Custom condition resolved by oracle authority.
    OracleCustom,
}

/// ─── Market Status ────────────────────────────────────────────────
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketStatus {
    /// Market is accepting bets.
    #[default]
    Open,

    /// Market deadline passed, awaiting resolution.
    Closed,

    /// Outcome has been determined; settlement in progress.
    Resolved,

    /// Market was cancelled; refunds available.
    Cancelled,

    /// All settlements complete.
    Settled,
}

/// ─── Outcome ──────────────────────────────────────────────────────
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outcome {
    #[default]
    Unresolved,
    Yes,
    No,
}

/// ─── Bet Side ─────────────────────────────────────────────────────
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BetSide {
    Yes,
    No,
}

/// ─── User Position ────────────────────────────────────────────────
///
/// PDA: seeds = [b"position", market.key, user.key]
///
/// Tracks a user's bet in a specific market.
#[account]
#[derive(Default)]
pub struct UserPosition {
    /// The market this position belongs to.
    pub market: Pubkey,

    /// The user who owns this position.
    pub user: Pubkey,

    /// Side of the bet.
    pub side: BetSide,

    /// Total lamports deposited by the user.
    pub deposited: u64,

    /// Whether this position has been settled.
    pub settled: bool,

    /// Amount paid out (set after settlement).
    pub payout: u64,

    /// Bump seed.
    pub bump: u8,

    /// Reserved.
    pub _reserved: [u8; 32],
}

impl Default for BetSide {
    fn default() -> Self {
        BetSide::Yes
    }
}

impl UserPosition {
    pub const SIZE: usize = 8  // discriminator
        + 32                    // market
        + 32                    // user
        + 1                     // side
        + 8                     // deposited
        + 1                     // settled
        + 8                     // payout
        + 1                     // bump
        + 32;                   // reserved
}

/// ─── Global Config ────────────────────────────────────────────────
///
/// PDA: seeds = [b"config"]
///
/// Protocol-level settings.
#[account]
#[derive(Default)]
pub struct GlobalConfig {
    /// Protocol authority (can update config).
    pub authority: Pubkey,

    /// Protocol fee in basis points (e.g. 50 = 0.5%).
    pub fee_bps: u16,

    /// Fee collector wallet.
    pub fee_collector: Pubkey,

    /// Next market ID to assign.
    pub next_market_id: u64,

    /// Total markets created.
    pub total_markets: u64,

    /// Total volume processed (lamports).
    pub total_volume: u64,

    /// Bump seed.
    pub bump: u8,

    /// Reserved.
    pub _reserved: [u8; 128],
}

impl GlobalConfig {
    pub const SIZE: usize = 8  // discriminator
        + 32                    // authority
        + 2                     // fee_bps
        + 32                    // fee_collector
        + 8                     // next_market_id
        + 8                     // total_markets
        + 8                     // total_volume
        + 1                     // bump
        + 128;                  // reserved
}

