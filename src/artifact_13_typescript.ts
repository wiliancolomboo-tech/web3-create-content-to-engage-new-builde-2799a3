import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Solquest } from "../target/types/solquest";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("SolQuest: Developer Onboarding Verification Protocol", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Solquest as Program<Solquest>;

  const admin = (provider.wallet as anchor.Wallet).payer;
  const relayer = Keypair.generate();
  const builder = Keypair.generate();
  const maliciousActor = Keypair.generate();

  const questId = new anchor.BN(101);
  const rawSecretCode = "SolanaSealevelRuntime2025";
  const expectedHash = crypto.createHash("sha256").update(rawSecretCode).digest();
  const rewardAmount = new anchor.BN(0.5 * LAMPORTS_PER_SOL);
  const escrowDeposit = new anchor.BN(2.0 * LAMPORTS_PER_SOL);

  let configPda: PublicKey;
  let questPda: PublicKey;
  let vaultPda: PublicKey;
  let progressPda: PublicKey;

  before(async () => {
    // Fund test wallets
    const airdropRelayer = await provider.connection.requestAirdrop(relayer.publicKey, 5 * LAMPORTS_PER_SOL);
    const airdropBuilder = await provider.connection.requestAirdrop(builder.publicKey, 1 * LAMPORTS_PER_SOL);
    const airdropMallory = await provider.connection.requestAirdrop(maliciousActor.publicKey, 1 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(airdropRelayer);
    await provider.connection.confirmTransaction(airdropBuilder);
    await provider.connection.confirmTransaction(airdropMallory);

    [configPda] = PublicKey.findProgramAddressSync([Buffer.from("protocol_config")], program.programId);
    [questPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("quest"), questId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("quest_vault"), questId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
    [progressPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("builder_progress"), questId.toArrayLike(Buffer, "le", 8), builder.publicKey.toBuffer()],
      program.programId
    );
  });

  it("1. [Sanity] Initializes Protocol Configuration", async () => {
    const tx = await program.methods
      .initialize()
      .accounts({
        config: configPda,
        admin: admin.publicKey,
        feeRelayer: relayer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const configAccount = await program.account.protocolConfig.fetch(configPda);
    expect(configAccount.admin.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(configAccount.feeRelayer.toBase58()).to.equal(relayer.publicKey.toBase58());
    expect(configAccount.totalQuests.toNumber()).to.equal(0);
    expect(configAccount.isPaused).to.be.false;
  });

  it("2. [Admin] Deploys a new Coding Quest with Funded Escrow Vault", async () => {
    const now = Math.floor(Date.now() / 1000);
    const expiresAt = new anchor.BN(now + 3600); // 1 hour duration
    const maxCompletions = 4;

    await program.methods
      .createQuest(
        questId,
        Array.from(expectedHash),
        rewardAmount,
        expiresAt,
        maxCompletions,
        escrowDeposit
      )
      .accounts({
        config: configPda,
        quest: questPda,
        vault: vaultPda,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const questAccount = await program.account.quest.fetch(questPda);
    expect(questAccount.questId.toNumber()).to.equal(questId.toNumber());
    expect(questAccount.rewardAmount.toNumber()).to.equal(rewardAmount.toNumber());
    expect(questAccount.currentCompletions).to.equal(0);

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.equal(escrowDeposit.toNumber());
  });

  it("3. [Negative Security] Rejects Invalid Solution Preimage", async () => {
    const wrongPreimage = Buffer.from("IncorrectExecutionPayload");
    const score = 85;

    try {
      await program.methods
        .submitSolution(questId, wrongPreimage, score)
        .accounts({
          config: configPda,
          quest: questPda,
          progress: progressPda,
          builder: builder.publicKey,
          payer: relayer.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([relayer])
        .rpc();
      expect.fail("Submission should have failed due to invalid hash");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("InvalidSolutionPreimage");
    }
  });

  it("4. [Relayer Sponsored] Submits Correct Preimage on Builder's Behalf", async () => {
    const validPreimage = Buffer.from(rawSecretCode);
    const score = 100;

    // Relayer signs as payer, while builder gets credited
    const tx = await program.methods
      .submitSolution(questId, validPreimage, score)
      .accounts({
        config: configPda,
        quest: questPda,
        progress: progressPda,
        builder: builder.publicKey,
        payer: relayer.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([relayer])
      .rpc();

    const progressAccount = await program.account.builderProgress.fetch(progressPda);
    expect(progressAccount.builder.toBase58()).to.equal(builder.publicKey.toBase58());
    expect(progressAccount.executionScore).to.equal(100);
    expect(progressAccount.rewardClaimed).to.be.false;

    const questAccount = await program.account.quest.fetch(questPda);
    expect(questAccount.currentCompletions).to.equal(1);
  });

  it("5. [Reward Claim] Builder Claims SOL Bounty via PDA Transfer", async () => {
    const initialBuilderBalance = await provider.connection.getBalance(builder.publicKey);

    await program.methods
      .claimReward(questId)
      .accounts({
        config: configPda,
        quest: questPda,
        vault: vaultPda,
        progress: progressPda,
        builder: builder.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([builder])
      .rpc();

    const progressAccount = await program.account.builderProgress.fetch(progressPda);
    expect(progressAccount.rewardClaimed).to.be.true;

    const finalBuilderBalance = await provider.connection.getBalance(builder.publicKey);
    // Allow for small transaction fee deduction
    expect(finalBuilderBalance).to.be.greaterThan(initialBuilderBalance);
  });

  it("6. [Negative Security] Prevents Double-Claiming of Escrow Bounty", async () => {
    try {
      await program.methods
        .claimReward(questId)
        .accounts({
          config: configPda,
          quest: questPda,
          vault: vaultPda,
          progress: progressPda,
          builder: builder.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([builder])
        .rpc();
      expect.fail("Expected second claim attempt to fail");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("QuestAlreadyCompleted");
    }
  });
});