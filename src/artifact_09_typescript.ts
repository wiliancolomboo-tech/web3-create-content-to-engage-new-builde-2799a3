// File: /tests/builder_ignition.ts
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BuilderIgnition } from "../target/types/builder_ignition";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { PublicKey, Keypair, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("Solana Builder Ignition Protocol E2E Verification", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BuilderIgnition as Program<BuilderIgnition>;

  const organizer = Keypair.generate();
  const builderOne = Keypair.generate();
  const maliciousActor = Keypair.generate();

  let rewardMint: PublicKey;
  let organizerTokenAccount: PublicKey;
  let builderOneTokenAccount: PublicKey;
  let hackathonConfigPda: PublicKey;
  let escrowVaultPda: PublicKey;
  let builderOnePda: PublicKey;

  const MILESTONE_REWARD = new anchor.BN(50_000_000); // 50 Tokens (e.g., 50 USDC at 6 decimals)
  const ESCROW_FUND_AMOUNT = 500_000_000; // 500 Tokens

  before(async () => {
    // 1. Airdrop SOL to actors
    for (const actor of [organizer, builderOne, maliciousActor]) {
      const sig = await provider.connection.requestAirdrop(
        actor.publicKey,
        10 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig, "confirmed");
    }

    // 2. Initialize SPL Token Mint
    rewardMint = await createMint(
      provider.connection,
      organizer,
      organizer.publicKey,
      null,
      6
    );

    // 3. Create Organizer Associated Token Account & Mint Reward Reserves
    organizerTokenAccount = await createAccount(
      provider.connection,
      organizer,
      rewardMint,
      organizer.publicKey
    );

    await mintTo(
      provider.connection,
      organizer,
      rewardMint,
      organizerTokenAccount,
      organizer,
      ESCROW_FUND_AMOUNT
    );

    // 4. Create Builder Token Account
    builderOneTokenAccount = await createAccount(
      provider.connection,
      builderOne,
      rewardMint,
      builderOne.publicKey
    );

    // 5. Derive PDAs
    [hackathonConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("hackathon"), organizer.publicKey.toBuffer()],
      program.programId
    );

    [escrowVaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("escrow"), hackathonConfigPda.toBuffer()],
      program.programId
    );

    [builderOnePda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("builder"),
        hackathonConfigPda.toBuffer(),
        builderOne.publicKey.toBuffer(),
      ],
      program.programId
    );
  });

  it("1. Initializes the Hackathon workspace and Escrow Vault", async () => {
    const tx = await program.methods
      .initializeHackathon(MILESTONE_REWARD)
      .accounts({
        hackathonConfig: hackathonConfigPda,
        rewardMint: rewardMint,
        escrowVault: escrowVaultPda,
        organizer: organizer.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([organizer])
      .rpc();

    // Fund the Escrow Vault PDA directly
    await mintTo(
      provider.connection,
      organizer,
      rewardMint,
      escrowVaultPda,
      organizer,
      ESCROW_FUND_AMOUNT
    );

    const configAccount = await program.account.hackathonConfig.fetch(
      hackathonConfigPda
    );
    expect(configAccount.organizer.toBase58()).to.equal(
      organizer.publicKey.toBase58()
    );
    expect(configAccount.milestoneRewardAmount.toString()).to.equal(
      MILESTONE_REWARD.toString()
    );
    expect(configAccount.isActive).to.be.true;

    const vaultAccount = await getAccount(provider.connection, escrowVaultPda);
    expect(Number(vaultAccount.amount)).to.equal(ESCROW_FUND_AMOUNT);
  });

  it("2. Registers a new Hackathon Builder (allocating BuilderRecord PDA)", async () => {
    await program.methods
      .registerBuilder()
      .accounts({
        hackathonConfig: hackathonConfigPda,
        builderRecord: builderOnePda,
        builder: builderOne.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([builderOne])
      .rpc();

    const record = await program.account.builderRecord.fetch(builderOnePda);
    expect(record.builder.toBase58()).to.equal(builderOne.publicKey.toBase58());
    expect(record.status).to.equal(0); // STATUS_UNSUBMITTED
  });

  it("3. Builder submits project milestone with SHA-256 payload hash", async () => {
    const rawCommitHash = crypto
      .createHash("sha256")
      .update("https://github.com/solana-builder/hackathon-submission-v1")
      .digest();
    const submissionArray = Array.from(new Uint8Array(rawCommitHash));

    await program.methods
      .submitMilestone(submissionArray)
      .accounts({
        hackathonConfig: hackathonConfigPda,
        builderRecord: builderOnePda,
        builder: builderOne.publicKey,
      })
      .signers([builderOne])
      .rpc();

    const record = await program.account.builderRecord.fetch(builderOnePda);
    expect(record.status).to.equal(1); // STATUS_SUBMITTED
    expect(Buffer.from(record.submissionHash).toString("hex")).to.equal(
      rawCommitHash.toString("hex")
    );
  });

  it("4. Prevents unauthorized evaluator from disbursing funds", async () => {
    try {
      await program.methods
        .verifyAndDisburse()
        .accounts({
          hackathonConfig: hackathonConfigPda,
          builderRecord: builderOnePda,
          escrowVault: escrowVaultPda,
          builderTokenAccount: builderOneTokenAccount,
          organizer: maliciousActor.publicKey,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        })
        .signers([maliciousActor])
        .rpc();
      expect.fail("Should have failed with unauthorized signer constraint.");
    } catch (err: any) {
      expect(err.toString()).to.include("ConstraintRaw") ||
      expect(err.toString()).to.include("has_one constraint was violated");
    }
  });

  it("5. Organizer verifies milestone and executes PDA-signed token transfer", async () => {
    const balanceBefore = await getAccount(
      provider.connection,
      builderOneTokenAccount
    );
    expect(Number(balanceBefore.amount)).to.equal(0);

    await program.methods
      .verifyAndDisburse()
      .accounts({
        hackathonConfig: hackathonConfigPda,
        builderRecord: builderOnePda,
        escrowVault: escrowVaultPda,
        builderTokenAccount: builderOneTokenAccount,
        organizer: organizer.publicKey,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .signers([organizer])
      .rpc();

    const balanceAfter = await getAccount(
      provider.connection,
      builderOneTokenAccount
    );
    expect(Number(balanceAfter.amount)).to.equal(MILESTONE_REWARD.toNumber());

    const record = await program.account.builderRecord.fetch(builderOnePda);
    expect(record.status).to.equal(2); // STATUS_VERIFIED
  });

  it("6. Prevents double-claiming or re-verification of already disbursed milestone", async () => {
    try {
      await program.methods
        .verifyAndDisburse()
        .accounts({
          hackathonConfig: hackathonConfigPda,
          builderRecord: builderOnePda,
          escrowVault: escrowVaultPda,
          builderTokenAccount: builderOneTokenAccount,
          organizer: organizer.publicKey,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        })
        .signers([organizer])
        .rpc();
      expect.fail("Should have rejected double disbursement");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidStatusTransition");
    }
  });

  it("7. Sweeps remaining vault balance and closes Hackathon PDA cleanly", async () => {
    await program.methods
      .closeHackathon()
      .accounts({
        hackathonConfig: hackathonConfigPda,
        escrowVault: escrowVaultPda,
        organizerTokenAccount: organizerTokenAccount,
        organizer: organizer.publicKey,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
      })
      .signers([organizer])
      .rpc();

    // Verify Config PDA is zeroed and closed
    const accountInfo = await provider.connection.getAccountInfo(
      hackathonConfigPda
    );
    expect(accountInfo).to.be.null;
  });
});