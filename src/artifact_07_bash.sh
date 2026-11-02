# 1. Create base project directory
mkdir -p builder_quest_engine && cd builder_quest_engine

# 2. Initialize fresh Anchor project using latest standards
anchor init builder_quest --template multiple-workspace
cd builder_quest

# 3. Add necessary dependencies to Anchor workspace
yarn add -D typescript mocha chai @types/mocha @types/chai ts-node @coral-xyz/anchor @solana/web3.js @solana/spl-token