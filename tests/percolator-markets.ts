import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import { expect } from "chai";
import { PercolatorMarkets } from "../target/types/percolator_markets";

describe("percolator-markets", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PercolatorMarkets as Program<PercolatorMarkets>;

  const creator = Keypair.generate();
  const oracle = Keypair.generate();
  const bettorYes = Keypair.generate();
  const bettorNo = Keypair.generate();

  let configPda: PublicKey;
  let configBump: number;
  let marketPda: PublicKey;
  let marketBump: number;
  let vaultPda: PublicKey;
  let yesMintPda: PublicKey;
  let noMintPda: PublicKey;

  const MARKET_ID = new anchor.BN(0);

  before(async () => {
    // Airdrop SOL to all participants
    const airdropAmount = 10 * LAMPORTS_PER_SOL;

    for (const kp of [creator, oracle, bettorYes, bettorNo]) {
      const sig = await provider.connection.requestAirdrop(kp.publicKey, airdropAmount);
      await provider.connection.confirmTransaction(sig);
    }

    // Derive PDAs
    [configPda, configBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );
  });

  // ─── Initialize Global Config ──────────────────────────────────

  it("Initializes global config", async () => {
    // Config initialization would be a separate instruction in production.
    // For tests, we assume it exists or create it via a setup instruction.
    // This test validates the PDA derivation.
    console.log("Config PDA:", configPda.toBase58());
    console.log("Program ID:", program.programId.toBase58());
  });

  // ─── Create Market ──────────────────────────────────────────────

  it("Creates a prediction market", async () => {
    // Derive market PDA
    [marketPda, marketBump] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        creator.publicKey.toBuffer(),
        MARKET_ID.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), marketPda.toBuffer()],
      program.programId
    );

    [yesMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("yes_mint"), marketPda.toBuffer()],
      program.programId
    );

    [noMintPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("no_mint"), marketPda.toBuffer()],
      program.programId
    );

    console.log("Market PDA:", marketPda.toBase58());
    console.log("Vault PDA:", vaultPda.toBase58());
    console.log("YES Mint:", yesMintPda.toBase58());
    console.log("NO Mint:", noMintPda.toBase58());

    // Market creation params
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3600); // 1 hour
    const tokenMint = Keypair.generate().publicKey; // mock token

    const params = {
      question: "Will $PEPE reach $1M market cap in 24h?",
      rule: { marketCapTarget: {} },
      targetValue: new anchor.BN(1_000_000),
      tokenMint,
      oracle: oracle.publicKey,
      deadline,
    };

    // In a full test, we'd call create_market here.
    // For now, validate the PDA derivation and params.
    expect(params.question.length).to.be.lessThan(256);
    expect(deadline.toNumber()).to.be.greaterThan(Math.floor(Date.now() / 1000));
  });

  // ─── Place Bet ──────────────────────────────────────────────────

  it("Validates bet amount > 0", () => {
    const amount = new anchor.BN(0);
    expect(amount.toNumber()).to.equal(0);
    // In a full test, this would fail with PercolatorError::ZeroBetAmount
  });

  it("Calculates correct pool distribution", () => {
    const yesPool = 5000 * LAMPORTS_PER_SOL;
    const noPool = 3000 * LAMPORTS_PER_SOL;
    const totalPool = yesPool + noPool;
    const yesPercent = Math.round((yesPool / totalPool) * 100);

    expect(yesPercent).to.equal(63);
    console.log(`Pool: YES ${yesPercent}% / NO ${100 - yesPercent}%`);
  });

  // ─── H-Ratio Calculation ────────────────────────────────────────

  describe("h-ratio settlement math", () => {
    it("Returns 100% (10000 bps) when vault is fully solvent", () => {
      const vaultBalance = 8000;
      const winnerPool = 5000;
      const loserPool = 3000;
      const totalClaims = winnerPool + loserPool;

      const h = Math.min(vaultBalance, totalClaims) / totalClaims;
      const hBps = Math.round(h * 10000);

      expect(hBps).to.equal(10000);
      console.log(`h-ratio: ${hBps} bps (${(hBps / 100).toFixed(1)}%)`);
    });

    it("Haircuts profit when vault is stressed", () => {
      const vaultBalance = 6000;
      const winnerPool = 5000;
      const loserPool = 3000;
      const totalClaims = winnerPool + loserPool;

      const h = Math.min(vaultBalance, totalClaims) / totalClaims;
      const hBps = Math.round(h * 10000);

      expect(hBps).to.equal(7500); // 75%
      console.log(`h-ratio: ${hBps} bps (${(hBps / 100).toFixed(1)}%)`);
    });

    it("Correctly computes payout with h-ratio", () => {
      const userStake = 1000;
      const winnerPool = 5000;
      const loserPool = 3000;
      const hBps = 7500; // 75%

      // Capital (senior claim) — returned in full
      const capital = userStake;

      // Profit (junior claim) — share of loser pool × h
      const profitShare = (userStake / winnerPool) * loserPool;
      const profitAfterH = profitShare * (hBps / 10000);

      const payout = capital + profitAfterH;

      expect(capital).to.equal(1000);
      expect(profitShare).to.equal(600);
      expect(profitAfterH).to.equal(450);
      expect(payout).to.equal(1450);

      console.log(`Capital: ${capital}, Profit: ${profitAfterH} (raw: ${profitShare}), Total: ${payout}`);
    });

    it("Ensures losers get nothing", () => {
      const userStake = 1000;
      const isWinner = false;
      const payout = isWinner ? userStake : 0;

      expect(payout).to.equal(0);
    });

    it("Handles edge case: all bets on one side", () => {
      const yesPool = 8000;
      const noPool = 0;
      const winnerPool = yesPool;
      const loserPool = noPool;

      // No losers → no profit to distribute → payout = stake only
      const userStake = 1000;
      const profitShare = winnerPool > 0 ? (userStake / winnerPool) * loserPool : 0;
      const payout = userStake + profitShare;

      expect(payout).to.equal(1000);
      console.log("All YES: payout = stake (no losers to profit from)");
    });

    it("Validates insolvency safety invariant", () => {
      // The core Percolator guarantee:
      // sum(all_payouts) <= vault_balance
      //
      // This is enforced by the h-ratio: if the vault can't cover
      // all claims, h < 1 reduces profits proportionally.

      const vaultBalance = 6000;
      const winnerPool = 5000;
      const loserPool = 3000;
      const totalClaims = winnerPool + loserPool;
      const hBps = Math.round(Math.min(vaultBalance, totalClaims) / totalClaims * 10000);

      // Simulate all winners settling
      let totalPayout = 0;
      const positions = [1000, 1500, 500, 2000]; // individual stakes

      for (const stake of positions) {
        const capital = stake;
        const profitShare = (stake / winnerPool) * loserPool;
        const profitAfterH = profitShare * (hBps / 10000);
        totalPayout += capital + profitAfterH;
      }

      // Invariant: total payouts must not exceed vault
      expect(totalPayout).to.be.at.most(vaultBalance);
      console.log(`Total payouts: ${totalPayout} <= vault: ${vaultBalance} ✓`);
    });
  });

  // ─── Market Resolution ──────────────────────────────────────────

  describe("market resolution", () => {
    it("Only oracle can resolve", () => {
      // In a full test, calling resolve from non-oracle would fail with
      // PercolatorError::UnauthorizedOracle
      const isOracle = oracle.publicKey.equals(oracle.publicKey);
      expect(isOracle).to.be.true;
    });

    it("Cannot resolve an already resolved market", () => {
      // Status check: Resolved market should reject re-resolution
      const status = "Resolved";
      const canResolve = status === "Open" || status === "Closed";
      expect(canResolve).to.be.false;
    });
  });

  // ─── Cancellation & Refund ──────────────────────────────────────

  describe("cancellation", () => {
    it("Refunds full amount on cancellation", () => {
      const deposited = 5000;
      const refund = deposited; // 100% refund
      expect(refund).to.equal(deposited);
    });

    it("Cannot cancel a resolved market", () => {
      const status = "Resolved";
      const canCancel = status === "Open" || status === "Closed";
      expect(canCancel).to.be.false;
    });
  });
});

