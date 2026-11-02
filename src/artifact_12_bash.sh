# Inspect generated PDA balance for Track 1 Escrow Vault
solana balance $(solana-keygen find-program-address --program-id BQst111111111111111111111111111111111111111 "vault" 0x0100000000000000)

# Stream transaction logs to observe instruction compute consumption
solana logs BQst111111111111111111111111111111111111111