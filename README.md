# Percolator Markets

**Insolvency-safe binary prediction markets on Solana.**

Built on the [Percolator](https://github.com/aeyakovenko/percolator) risk engine by Anatoly Yakovenko — adapted from perpetual futures settlement to binary prediction markets for PumpSwap-migrated tokens.

## How It Works

Users bet **YES** or **NO** on token events (e.g., "Will $TOKEN reach $1M market cap?"). When the market resolves, winners are paid using Percolator's two-claim settlement model:

| Claim | Priority | Description |
|-------|----------|-------------|
| **Capital** | Senior | Original stake — returned first |
| **Profit** | Junior | Share of losing pool — subject to h-ratio |

### The h-ratio

```
h = min(vault_balance, total_claims) / total_claims
```

- `h = 1.0` → full payout, vault is solvent
- `h < 1.0` → profits are haircut proportionally

**The market can never become insolvent.** This is the core safety guarantee inherited from Percolator.

## Architecture

```
programs/
└── percolator-markets/
    └── src/
        ├── lib.rs                 # Program entrypoint & instruction dispatch
        ├── state.rs               # Account structures (Market, Position, Config)
        ├── errors.rs              # Custom error codes
        └── instructions/
            ├── create_market.rs   # Create binary market with vault + YES/NO mints
            ├── place_bet.rs       # Deposit SOL → vault, mint position tokens
            ├── resolve.rs         # Oracle resolves outcome, compute h-ratio
            ├── settle.rs          # Two-claim payout (Capital + Profit × h)
            ├── cancel.rs          # Cancel market (creator/oracle)
            └── claim_refund.rs    # Full refund from cancelled markets
```

## Instructions

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `create_market` | Creator | Deploy new market with question, deadline, oracle |
| `place_bet` | Bettor | Deposit SOL, receive YES/NO position tokens |
| `resolve_market` | Oracle | Set outcome (YES/NO), compute h-ratio |
| `settle` | Winner | Claim payout: capital + profit × h |
| `cancel_market` | Creator/Oracle | Cancel market before resolution |
| `claim_refund` | User | Refund from cancelled market |

## Accounts

### Market (PDA)
```
seeds = ["market", creator, market_id]
```
Core market state: pools, outcome, h-ratio, vault reference.

### Vault (PDA)
```
seeds = ["vault", market]
```
Holds all SOL deposits. Only the program can withdraw.

### UserPosition (PDA)
```
seeds = ["position", market, user]
```
Tracks individual bets: side, amount deposited, settlement status.

### YES/NO Mints (PDA)
```
seeds = ["yes_mint", market] / ["no_mint", market]
```
SPL token mints — market is authority. 1 token = 1 lamport deposited.

## Settlement Math

```
winner_pool = total YES (or NO) deposits
loser_pool  = total NO (or YES) deposits

For each winner:
  capital     = user_stake
  profit      = (user_stake / winner_pool) × loser_pool
  h           = min(vault, total_claims) / total_claims
  payout      = capital + profit × h

Invariant: Σ payouts ≤ vault_balance  (always)
```

## Market Eligibility

Only tokens that have **migrated to PumpSwap** are eligible. This ensures:
- Real liquidity exists
- Price feeds are available
- Token has passed initial bonding curve

## Build & Test

```bash
# Install dependencies
yarn install

# Build the program
anchor build

# Run tests
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

## Status

🟡 **Devnet** — Program deployed to Solana devnet for testing.

Program ID: `PERCmkT7XHFjnMGiLBKo9Qxmf4BBJY6oSVhLwMpXuq`

## Links

- [Percolator Core](https://github.com/aeyakovenko/percolator) — Original risk engine
- [Risk Engine Fork](https://github.com/percmarket/percolator-predictions) — Prediction market adaptation
- [DexScreener](https://dexscreener.com) — Token data source

## License

Apache-2.0

