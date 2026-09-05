use anchor_lang::prelude::*;
use crate::state::*;
use crate::error::SolQuestError;

#[derive(Accounts)]
#[instruction(quest_id: u64)]
pub struct ClaimReward<'info> {
    #[account(
        seeds = [PROTOCOL_CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, ProtocolConfig>,

    #[account(
        seeds = [QUEST_SEED, quest_id.to_le_bytes().as_ref()],
        bump = quest.bump
    )]
    pub quest: Account<'info, Quest>,

    /// CHECK: Vault account holding escrowed rewards
    #[account(
        mut,
        seeds = [VAULT_SEED, quest_id.to_le_bytes().as_ref()],
        bump = quest.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [PROGRESS_SEED, quest_id.to_le_bytes().as_ref(), builder.key().as_ref()],
        bump = progress.bump,
        has_one = builder,
        constraint = !progress.reward_claimed @ SolQuestError::QuestAlreadyCompleted
    )]
    pub progress: Account<'info, BuilderProgress>,

    #[account(mut)]
    pub builder: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimReward>, quest_id: u64) -> Result<()> {
    require!(!ctx.accounts.config.is_paused, SolQuestError::ProtocolPaused);
    let quest = &ctx.accounts.quest;
    let reward = quest.reward_amount;

    if reward > 0 {
        let vault_balance = ctx.accounts.vault.lamports();
        require!(vault_balance >= reward, SolQuestError::InsufficientVaultBalance);

        // Signer seed derivation for the Vault PDA
        let quest_id_bytes = quest_id.to_le_bytes();
        let vault_bump = ctx.accounts.quest.vault_bump;
        let signer_seeds: &[&[&[u8]]] = &[&[
            VAULT_SEED,
            quest_id_bytes.as_ref(),
            &[vault_bump],
        ]];

        // Direct Lamport debit & credit ensuring rent consistency
        **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .vault
            .to_account_info()
            .lamports()
            .checked_sub(reward)
            .ok_or(SolQuestError::ArithmeticOverflow)?;

        **ctx.accounts.builder.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .builder
            .to_account_info()
            .lamports()
            .checked_add(reward)
            .ok_or(SolQuestError::ArithmeticOverflow)?;
    }

    let progress = &mut ctx.accounts.progress;
    progress.reward_claimed = true;

    Ok(())
}