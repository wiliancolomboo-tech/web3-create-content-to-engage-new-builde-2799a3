import * as anchor from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import * as crypto from "crypto";

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.BuilderQuest;

  const challengeIndex = 0;
  const builderPubkey = provider.wallet.publicKey;

  // Derive proof on-the-fly
  const seed = Buffer.from("SOLANA_HACKATHON_QUEST_2025");
  const hasher = crypto.createHash("sha256");
  hasher.update(Buffer.from([challengeIndex]));
  hasher.update(builderPubkey.toBuffer());
  hasher.update(seed);
  const proof = hasher.digest();

  console.log(`Submitting solution for Builder: ${builderPubkey.toBase58()}...`);
  const tx = await program.methods
    .verifyChallenge(challengeIndex, Array.from(proof))
    .rpc();

  console.log(`Challenge #0 verified successfully! TX Signature: ${tx}`);
}

main().catch(console.error);