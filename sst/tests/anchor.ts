//This file needs to be edited 
import * as anchor from "@coral-xyz/anchor";
import BN from "bn.js";
import assert from "assert";
import * as web3 from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  createAssociatedTokenAccount,
} from "@solana/spl-token";
import type { Sst } from "../target/types/sst";

describe("sst tests", () => {
  // Set the provider to the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Sst as anchor.Program<Sst>;

  // Global variables for tests.
  let mint: web3.PublicKey;
  let stakerTokenAccount: web3.PublicKey;
  let vaultTokenAccount: web3.PublicKey;
  let vaultAuthority: web3.PublicKey;

  // We use the provider wallet as our staker.
  const staker = provider.wallet;

  before(async () => {
    // Create a new mint for our $SST token.
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      staker.publicKey, // mint authority
      null,            // freeze authority
      6                // decimals
    );

    // Create an associated token account for the staker and mint some tokens.
    const stakerTokenAccountObj = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      staker.publicKey
    );
    stakerTokenAccount = stakerTokenAccountObj.address;
    await mintTo(
      provider.connection,
      provider.wallet.payer,
      mint,
      stakerTokenAccount,
      staker.publicKey,
      1000000 // amount to mint (adjust as needed)
    );

    // Derive the vault authority PDA from seed "vault".
    [vaultAuthority] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("vault")],
      program.programId
    );

    // Create an associated token account for the vault authority.
    vaultTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      vaultAuthority
    );
  });

  it("Stake tokens (standard, no lock)", async () => {
    const stakeAmount = new BN(1000);

    // Derive the PDA for the staking record using seed ["stake", staker public key].
    const [stakeInfoPda] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("stake"), staker.publicKey.toBuffer()],
      program.programId
    );

    // Call the stake instruction.
    await program.methods
      .stake(stakeAmount)
      .accounts({
        staker: staker.publicKey,
        stakeInfo: stakeInfoPda,
        stakerTokenAccount: stakerTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        vaultAuthority: vaultAuthority,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
        rent: web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    // Fetch the stake_info account to verify initialization.
    const stakeInfoAccount = await program.account.stakeInfo.fetch(stakeInfoPda);
    console.log("StakeInfo account:", stakeInfoAccount);
    assert.ok(stakeInfoAccount.staker.equals(staker.publicKey));
    assert.ok(new BN(stakeInfoAccount.amount).eq(stakeAmount));
  });

  it("Unstake tokens", async () => {
    const unstakeAmount = new BN(500);

    // Derive the same PDA for the stake_info account.
    const [stakeInfoPda] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("stake"), staker.publicKey.toBuffer()],
      program.programId
    );

    // Call the unstake instruction.
    await program.methods
      .unstake(unstakeAmount)
      .accounts({
        staker: staker.publicKey,
        stakeInfo: stakeInfoPda,
        stakerTokenAccount: stakerTokenAccount,
        vaultTokenAccount: vaultTokenAccount,
        vaultAuthority: vaultAuthority,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Fetch the stake_info account and verify the remaining balance.
    const stakeInfoAccount = await program.account.stakeInfo.fetch(stakeInfoPda);
    const expectedAmount = new BN(1000).sub(unstakeAmount);
    assert.ok(new BN(stakeInfoAccount.amount).eq(expectedAmount));
  });

  it("Execute trade with bonus incentives", async () => {
    // Derive the PDA for the stake_info account.
    const [stakeInfoPda] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("stake"), staker.publicKey.toBuffer()],
      program.programId
    );

    // Call execute_trade with an order execution time less than or equal to 100ms.
    await program.methods
      .executeTrade(new BN(80))
      .accounts({
        staker: staker.publicKey,
        stakeInfo: stakeInfoPda,
      })
      .rpc();

    // (Check the logs for confirmation; further state changes can be asserted if you expand the logic.)
  });

  it("Claim rewards (auto-compound with LP boost)", async () => {
    // Derive the PDA for the stake_info account.
    const [stakeInfoPda] = await web3.PublicKey.findProgramAddress(
      [Buffer.from("stake"), staker.publicKey.toBuffer()],
      program.programId
    );

    // Define liquidity provided.
    const liquidityProvided = new BN(10000);
    await program.methods
      .claimRewards(liquidityProvided)
      .accounts({
        staker: staker.publicKey,
        stakeInfo: stakeInfoPda,
        stakerTokenAccount: stakerTokenAccount,
        rewardVault: vaultTokenAccount,
        vaultAuthority: vaultAuthority,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    // Fetch and log the stake_info account after rewards are compounded.
    const stakeInfoAccount = await program.account.stakeInfo.fetch(stakeInfoPda);
    console.log("StakeInfo after claiming rewards:", stakeInfoAccount);
  });

  it("Create a governance proposal", async () => {
    // Derive the PDA for the proposal using seeds ["proposal", proposer, proposer].
    const [proposalPda] = await web3.PublicKey.findProgramAddress(
      [
        Buffer.from("proposal"),
        staker.publicKey.toBuffer(),
        staker.publicKey.toBuffer(),
      ],
      program.programId
    );

    const description = "Proposal for fee distribution changes";
    await program.methods
      .createProposal(description)
      .accounts({
        proposer: staker.publicKey,
        proposal: proposalPda,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    // Fetch the proposal account and verify its contents.
    const proposalAccount = await program.account.proposal.fetch(proposalPda);
    console.log("Proposal account:", proposalAccount);
    assert.ok(proposalAccount.proposer.equals(staker.publicKey));
    assert.equal(proposalAccount.description, description);
  });
});
