# Terminal 1: Launch Localnet Validator with pre-allocated slots
solana-test-validator --reset --quiet

# Terminal 2: Stream Program Logs
solana logs -u localhost QuestXv771111111111111111111111111111111111

# Terminal 3: Deploy & Execute Target Test Spec
anchor deploy --provider.cluster localhost
yarn run ts-mocha -p ./tsconfig.json tests/solquest.ts