# Set network context to Devnet
solana config set --url https://api.devnet.solana.com

# Ensure deployer wallet is funded
solana airdrop 2

# Build with release optimization flags
cargo build-sbf

# Deploy to Devnet
anchor deploy --provider.cluster Devnet