use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("QuestXv771111111111111111111111111111111111");

#[program]
pub mod solquest {
    use super::*;

    pub fn initialize(ctx: Context<InitializeProtocol>) -> Result<()> {
        instructions::initialize::handler(ctx)
    }

    pub fn create_quest(
        ctx: Context<CreateQuest>,
        quest_id: u64,
        expected_hash: [u8; 32],
        reward_amount: u64,
        expires_at: i64,
        max_completions: u32,
        escrow_deposit: u64,
    ) -> Result<()> {
        instructions::create_quest::handler(
            ctx,
            quest_id,
            expected_hash,
            reward_amount,
            expires_at,
            max_completions,
            escrow_deposit,
        )
    }

    pub fn submit_solution(
        ctx: Context<SubmitSolution>,
        quest_id: u64,
        solution_preimage: Vec<u8>,
        execution_score: u16,
    ) -> Result<()> {
        instructions::submit_solution::handler(ctx, quest_id, solution_preimage, execution_score)
    }

    pub fn claim_reward(ctx: Context<ClaimReward>, quest_id: u64) -> Result<()> {
        instructions::claim_reward::handler(ctx, quest_id)
    }
}