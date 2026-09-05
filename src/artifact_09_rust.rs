use anchor_lang::prelude::*;
use solana_program::hash::hashv;
use crate::state::*;
use crate::error::SolQuestError;

#[derive(Accounts)]
#[instruction(quest_id: u64)]
pub struct SubmitSolution<'info> {
    #[account(
        seeds = [PROTOCOL_CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        mut,
        seeds = [QUEST_SEED, quest_id.to_le_bytes().as_ref()],
        bump = quest.bump,
        has_one = is_active
    )]
    pub quest: Account<'info, Quest>,

    #[account(
        init,
        payer = payer,
        space = BuilderProgress::LEN,
        seeds = [PROGRESS_SEED, quest_id.to_le_bytes().as_ref(), builder.key().as_ref()],
        bump
    )]
    pub progress: Account<'info, BuilderProgress>,

    /// CHECK: The builder identity receiving credit
    pub builder: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<SubmitSolution>,
    quest_id: u64,
    solution_preimage: Vec<u8>,
    execution_score: u16,
) -> Result<()> {
    require!(!ctx.accounts.config.is_paused, SolQuestError::ProtocolPaused);
    let quest = &mut ctx.accounts.quest;

    let clock = Clock::get()?;
    require!(clock.unix_timestamp <= quest.expires_at, SolQuestError::QuestInactive);
    require!(quest.current_completions < quest.max_completions, SolQuestError::QuestInactive);

    // Cryptographic validation: Hash preimage using SHA-256
    let calculated_hash = hashv(&[&solution_preimage]).to_bytes();
    require!(calculated_hash == quest.expected_hash, SolQuestError::InvalidSolutionPreimage);

    // Update quest statistics
    quest.current_completions = quest.current_completions
        .checked_add(1)
        .ok_or(SolQuestError::ArithmeticOverflow)?;

    // Store state on the Soulbound Progress account
    let progress = &mut ctx.accounts.progress;
    progress.builder = ctx.accounts.builder.key();
    progress.quest_id = quest_id;
    progress.completed_at = clock.unix_timestamp;
    progress.reward_claimed = false;
    progress.execution_score = execution_score;
    progress.bump = ctx.bumps.progress;

    Ok(())
}