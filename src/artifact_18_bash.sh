# 1. Install TypeScript dependencies
npm install @coral-xyz/anchor @solana/web3.js chai mocha @types/chai @types/mocha ts-mocha typescript

# 2. Build the Anchor Rust program
anchor build

# 3. Synchronize program keys
anchor keys sync

# 4. Run the full end-to-end integration and security test suite
anchor test