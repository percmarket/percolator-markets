use anchor_lang::prelude::*;

/// Custom error codes for the Percolator Markets program.
///
/// Error codes are offset from 6000 (Anchor convention).
#[error_code]
pub enum PercolatorError {
    /// Market is not in the expected status for this operation.
    #[msg("Market is not in the expected status")]
    InvalidMarketStatus,

    /// Market has not yet reached its deadline.
    #[msg("Market has not reached deadline")]
    MarketNotExpired,

    /// Market deadline has already passed; no more bets accepted.
    #[msg("Market deadline has passed")]
    MarketExpired,

    /// Only the designated oracle authority can resolve this market.
    #[msg("Unauthorized: not the oracle authority")]
    UnauthorizedOracle,

    /// Only the market creator can perform this action.
    #[msg("Unauthorized: not the market creator")]
    UnauthorizedCreator,

    /// Bet amount must be greater than zero.
    #[msg("Bet amount must be > 0")]
    ZeroBetAmount,

    /// Bet amount exceeds the maximum allowed per position.
    #[msg("Bet amount exceeds maximum")]
    BetAmountExceedsMax,

    /// Position has already been settled.
    #[msg("Position already settled")]
    AlreadySettled,

    /// Market has already been resolved.
    #[msg("Market already resolved")]
    AlreadyResolved,

    /// User has no position in this market.
    #[msg("No position found")]
    NoPosition,

    /// Question exceeds maximum length (256 bytes).
    #[msg("Question too long (max 256 bytes)")]
    QuestionTooLong,

    /// Deadline must be in the future.
    #[msg("Deadline must be in the future")]
    DeadlineInPast,

    /// Overflow in arithmetic operation.
    #[msg("Arithmetic overflow")]
    Overflow,

    /// Vault balance insufficient (should never happen if invariants hold).
    #[msg("Vault insolvency detected — this should be impossible")]
    VaultInsolvency,

    /// Cannot cancel a market that has already been resolved.
    #[msg("Cannot cancel a resolved market")]
    CannotCancelResolved,

    /// The user bet on the losing side; no payout available.
    #[msg("Position is on losing side")]
    LosingSide,

    /// Invalid outcome value.
    #[msg("Invalid outcome")]
    InvalidOutcome,
}

