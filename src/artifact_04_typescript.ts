import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { BuilderQuest } from "../target/types/builder_quest";
import { Keypair, LAMPORTS_PER_SOL, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("builder_quest core flow and security vectors", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BuilderQuest as Program<BuilderQuest>;

  const authority = Keypair.generate();
  const oracle = Keypair.generate();
  const sponsor = Keypair.generate();
  const builder = Keypair.generate();
  const attacker = Keypair.generate();

  const trackId = new BN(1);
  const rewardAmount = new BN(0.5 * LAMPORTS_PER_SOL);
  const maxBuilders = 5;
  const dummyHash = Array.from(crypto.randomBytes(32));

  let configPda: anchor.web3.PublicKey;
  let trackPda: anchor.web3.PublicKey;
  let escrowVaultPda: anchor.web3.PublicKey;
  let builderProfilePda: anchor.web3.PublicKey;

  before(async () => {
    // Fund test wallets
    const airdropSigners = [authority, oracle, sponsor, builder, attacker];
    for (const signer of airdropSigners) {
      const sig = await provider.connection.requestAirdrop(
        signer.publicKey,
        5 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig, "confirmed");
    }

    [configPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );

    [trackPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("track"), trackId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    [escrowVaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), trackId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    [builderProfilePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("builder"), builder.publicKey.toBuffer()],
      program.programId
    );
  });

  it("T01: Initializes protocol configuration successfully", async () => {
    const tx = await program.methods
      .initializeProtocol(oracle.publicKey)
      .accounts({
        config: configPda,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const configAccount = await program.account.protocolConfig.fetch(configPda);
    expect(configAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(configAccount.verificationOracle.toBase58()).to.equal(oracle.publicKey.toBase58());
    expect(configAccount.totalTracks.toNumber()).to.equal(0);
  });

  it("T02: Sponsor creates a track and initializes vault PDA", async () => {
    await program.methods
      .createTrack(trackId, rewardAmount, maxBuilders, dummyHash)
      .accounts({
        config: configPda,
        authority: authority.publicKey,
        sponsor: sponsor.publicKey,
        track: trackPda,
        escrowVault: escrowVaultPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority, sponsor])
      .rpc();

    const trackAccount = await program.account.trackAccount.fetch(trackPda);
    expect(trackAccount.rewardPerBuilder.toString()).to.equal(rewardAmount.toString());
    expect(trackAccount.maxBuilders).to.equal(maxBuilders);
    expect(trackAccount.completedBuilders).to.equal(0);
  });

  it("T03: Sponsor funds the track escrow vault", async () => {
    const depositAmount = new BN(2.5 * LAMPORTS_PER_SOL);

    await program.methods
      .fundTrackEscrow(trackId, depositAmount)
      .accounts({
        track: trackPda,
        escrowVault: escrowVaultPda,
        funder: sponsor.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([sponsor])
      .rpc();

    const vaultBalance = await provider.connection.getBalance(escrowVaultPda);
    expect(vaultBalance).to.be.at.least(depositAmount.toNumber());
  });

  it("T04: Builder registers profile", async () => {
    await program.methods
      .registerBuilder(dummyHash)
      .accounts({
        config: configPda,
        builder: builder.publicKey,
        builderProfile: builderProfilePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([builder])
      .rpc();

    const profile = await program.account.builderProfile.fetch(builderProfilePda);
    expect(profile.builder.toBase58()).to.equal(builder.publicKey.toBase58());
    expect(profile.completedMask.toNumber()).to.equal(0);
    expect(profile.reputationPoints).to.equal(0);
  });

  it("T05: Builder completes quest milestone and receives lamports", async () => {
    const builderBalanceBefore = await provider.connection.getBalance(builder.publicKey);

    await program.methods
      .completeTrackMilestone(trackId, dummyHash)
      .accounts({
        config: configPda,
        track: trackPda,
        escrowVault: escrowVaultPda,
        builderProfile: builderProfilePda,
        builder: builder.publicKey,
        oracle: oracle.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([builder, oracle])
      .rpc();

    const builderBalanceAfter = await provider.connection.getBalance(builder.publicKey);
    const profile = await program.account.builderProfile.fetch(builderProfilePda);

    expect(builderBalanceAfter).to.be.greaterThan(builderBalanceBefore);
    expect(profile.completedMask.toNumber()).to.equal(1 << trackId.toNumber());
    expect(profile.reputationPoints).to.equal(100);
  });

  it("T06: Fails when builder attempts double claim (Replay Protection)", async () => {
    let failed = false;
    try {
      await program.methods
        .completeTrackMilestone(trackId, dummyHash)
        .accounts({
          config: configPda,
          track: trackPda,
          escrowVault: escrowVaultPda,
          builderProfile: builderProfilePda,
          builder: builder.publicKey,
          oracle: oracle.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([builder, oracle])
        .rpc();
    } catch (err: any) {
      failed = true;
      expect(err.error.errorCode.code).to.equal("TrackAlreadyCompleted");
    }
    expect(failed).to.be.true;
  });

  it("T07: Fails when unauthorized oracle signs milestone completion", async () => {
    let failed = false;
    try {
      await program.methods
        .completeTrackMilestone(trackId, dummyHash)
        .accounts({
          config: configPda,
          track: trackPda,
          escrowVault: escrowVaultPda,
          builderProfile: builderProfilePda,
          builder: builder.publicKey,
          oracle: attacker.publicKey, // Invalid oracle
          systemProgram: SystemProgram.programId,
        })
        .signers([builder, attacker])
        .rpc();
    } catch (err: any) {
      failed = true;
      expect(err.error.errorCode.code).to.equal("InvalidOracleSigner");
    }
    expect(failed).to.be.true;
  });
});