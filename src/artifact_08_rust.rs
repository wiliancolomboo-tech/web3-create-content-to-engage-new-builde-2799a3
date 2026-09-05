use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use crate::state::*;
use crate::error::SolQuestError;

#[derive(Accounts)]
#[instruction(quest_id: u64)]
pub struct CreateQuest<'info> {
    #[account(
        mut,
        seeds = [PROTOCOL_CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ SolQuestError::UnauthorizedAdmin
    )]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        init,
        payer = admin,
        space = Quest::LEN,
        seeds = [QUEST_SEED, quest_id.to_le_bytes().as_ref()],
        bump
    )]
    pub quest: Account<'info, Quest>,

    /// CHECK: Vault PDA to hold reward escrow
    #[account(
        mut,
        seeds = [VAULT_SEED, quest_id.to_le_bytes().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateQuest>,
    quest_id: u64,
    expected_hash: [u8; 32],
    reward_amount: u64,
    expires_at: i64,
    max_completions: u32,
    escrow_deposit: u64,
) -> Result<()> {
    require!(!ctx.accounts.config.is_paused, SolQuestError::ProtocolPaused);

    let quest = &mut ctx.accounts.quest;
    quest.quest_id = quest_id;
    quest.creator = ctx.accounts.admin.key();
    quest.expected_hash = expected_hash;
    quest.reward_amount = reward_amount;
    quest.expires_at = expires_at;
    quest.max_completions = max_completions;
    quest.current_completions = 0;
    quest.is_active = true;
    quest.bump = ctx.bumps.quest;
    quest.vault_bump = ctx.bumps.vault;

    let config = &mut ctx.accounts.config;
    config.total_quests = config.total_quests.checked_add(1).ok_or(SolQuestError::ArithmeticOverflow)?;

    if escrow_deposit > 0 {
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.admin.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, escrow_deposit)?;
    }

    Ok(())
}