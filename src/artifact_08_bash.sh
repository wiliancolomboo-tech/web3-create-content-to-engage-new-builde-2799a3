# Set cluster to Devnet
solana config set --url https://api.devnet.solana.com

# Airdrop SOL to your deployer wallet
solana airdrop 2

# Build optimized binary
anchor build --verifiable

# Deploy program to Devnet
anchor deploy --provider.cluster devnet

# Verify program on-chain
solana program show <PROGRAM_ID>