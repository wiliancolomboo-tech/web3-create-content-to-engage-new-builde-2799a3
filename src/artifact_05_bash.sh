# 1. Clone repository and install dependencies
git clone https://github.com/solana-bounties/quest-engine.git
cd quest-engine
yarn install

# 2. Generate program keypair if not already present
solana-keygen new -o target/deploy/quest_engine-keypair.json --no-bip39-passphrase --force

# 3. Synchronize Program ID with lib.rs and Anchor.toml
anchor keys sync