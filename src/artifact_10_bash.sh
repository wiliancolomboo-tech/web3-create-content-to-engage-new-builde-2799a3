# 1. Build the program to generate IDL and TypeScript artifacts
anchor build

# 2. Update the declared ID in lib.rs if a fresh keypair was generated
# anchor keys sync

# 3. Run the comprehensive test matrix with compute unit logging
anchor test