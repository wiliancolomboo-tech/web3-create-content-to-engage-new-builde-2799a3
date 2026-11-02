import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { QuestEngine } from "../target/types/quest_engine";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("Solana Quest Engine Test Matrix", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.QuestEngine as Program<QuestEngine>;

  const authority = Keypair.generate();
  const keeper = Keypair.generate();
  const builder = Keypair.generate();
  const unauthorizedUser = Keypair.generate();

  const hackathonId = new anchor.BN(42);
  const questId = 0;
  const rewardAmount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);
  const xpPoints = 250;
  const maxCompletions = 100;

  let configPda: PublicKey;
  let vaultPda: PublicKey;
  let questPda: PublicKey;
  let builderPda: PublicKey;

  before(async () => {
    // Fund test actors
    for (const kp of [authority, keeper, builder, unauthorizedUser]) {
      const sig = await provider.connection.requestAirdrop(kp.publicKey, 10 * LAMPORTS_PER_SOL);
      const latestBlockhash = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction({
        signature: sig,
        ...latestBlockhash,
      });
    }

    [configPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("hackathon"), hackathonId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), configPda.toBuffer()],
      program.programId
    );

    [questPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("quest"), configPda.toBuffer(), Buffer.from([questId])],
      program.programId
    );

    [builderPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("builder"), configPda.toBuffer(), builder.publicKey.toBuffer()],
      program.programId
    );
  });

  it("1. Initializes the Hackathon and Escrow Vault", async () => {
    await program.methods
      .initializeHackathon(hackathonId, "Solana Global Renaissance 2025", keeper.publicKey)
      .accounts({
        config: configPda,
        vault: vaultPda,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const configAccount = await program.account.hackathonConfig.fetch(configPda);
    expect(configAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(configAccount.verifierKeeper.toBase58()).to.equal(keeper.publicKey.toBase58());
    expect(configAccount.questCount).to.equal(0);

    // Fund the escrow vault with 5 SOL for quest bounties
    const fundTx = new anchor.web3.Transaction().add(
      SystemProgram.transfer({
        fromPubkey: authority.publicKey,
        toPubkey: vaultPda,
        lamports: 5 * LAMPORTS_PER_SOL,
      })
    );
    await provider.sendAndConfirm(fundTx, [authority]);

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.be.at.least(5 * LAMPORTS_PER_SOL);
  });

  it("2. Authority registers Quest 0 (Milestone: Deploy Anchor Program)", async () => {
    const dummyHash = crypto.createHash("sha256").update("deploy-anchor-quest").digest();

    await program.methods
      .createQuest(questId, rewardAmount, xpPoints, maxCompletions, Array.from(dummyHash))
      .accounts({
        config: configPda,
        quest: questPda,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const questAccount = await program.account.quest.fetch(questPda);
    expect(questAccount.questId).to.equal(questId);
    expect(questAccount.rewardAmount.toNumber()).to.equal(rewardAmount.toNumber());
    expect(questAccount.isActive).to.be.true;
    expect(questAccount.currentCompletions).to.equal(0);
  });

  it("3. Builder registers for Hackathon Onboarding Track", async () => {
    await program.methods
      .registerBuilder()
      .accounts({
        config: configPda,
        builder: builderPda,
        signer: builder.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([builder])
      .rpc();

    const builderAccount = await program.account.builderProfile.fetch(builderPda);
    expect(builderAccount.builder.toBase58()).to.equal(builder.publicKey.toBase58());
    expect(builderAccount.completedMask.toString()).to.equal("0");
    expect(builderAccount.totalXp.toNumber()).to.equal(0);
  });

  it("4. Verification Keeper dispatches milestone completion & unlocks escrow micro-bounty", async () => {
    const builderBalanceBefore = await provider.connection.getBalance(builder.publicKey);
    const mockProofSig = new Array(64).fill(7);

    await program.methods
      .completeQuestAndDisburse(questId, mockProofSig)
      .accounts({
        config: configPda,
        quest: questPda,
        builder: builderPda,
        builderRecipient: builder.publicKey,
        vault: vaultPda,
        verifierKeeper: keeper.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([keeper])
      .rpc();

    const builderAccount = await program.account.builderProfile.fetch(builderPda);
    // Bit 0 must be set
    expect(builderAccount.completedMask.toNumber()).to.equal(1);
    expect(builderAccount.totalXp.toNumber()).to.equal(xpPoints);

    const builderBalanceAfter = await provider.connection.getBalance(builder.publicKey);
    expect(builderBalanceAfter - builderBalanceBefore).to.equal(rewardAmount.toNumber());
  });

  it("5. Security Guard: Prevents double-claiming the same quest milestone (Reentrancy / Replay)", async () => {
    const mockProofSig = new Array(64).fill(7);
    try {
      await program.methods
        .completeQuestAndDisburse(questId, mockProofSig)
        .accounts({
          config: configPda,
          quest: questPda,
          builder: builderPda,
          builderRecipient: builder.publicKey,
          vault: vaultPda,
          verifierKeeper: keeper.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([keeper])
        .rpc();
      expect.fail("Double-claiming should have failed with QuestAlreadyCompleted");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("QuestAlreadyCompleted");
    }
  });

  it("6. Security Guard: Rejects non-keeper unauthorized verification attempts", async () => {
    const mockProofSig = new Array(64).fill(9);
    try {
      await program.methods
        .completeQuestAndDisburse(questId, mockProofSig)
        .accounts({
          config: configPda,
          quest: questPda,
          builder: builderPda,
          builderRecipient: builder.publicKey,
          vault: vaultPda,
          verifierKeeper: unauthorizedUser.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([unauthorizedUser])
        .rpc();
      expect.fail("Should have rejected unauthorized keeper");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("UnauthorizedKeeper");
    }
  });
});