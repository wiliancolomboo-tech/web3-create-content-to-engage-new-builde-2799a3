use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("QuestEng1ne11111111111111111111111111111111");

pub const MAX_QUESTS: usize = 128;
pub const CONFIG_SEED: &[u8] = b"hackathon";
pub const VAULT_SEED: &[u8] = b"vault";
pub const QUEST_SEED: &[u8] = b"quest";
pub const BUILDER_SEED: &[u8] = b"builder";

#[program]
pub mod quest_engine {
    use super::*;

    /// Initialize a new hackathon configuration and allocate the PDA-controlled escrow vault.
    pub fn initialize_hackathon(
        ctx: Context<InitializeHackathon>,
        hackathon_id: u64,
        title: String,
        verifier_keeper: Pubkey,
    ) -> Result<()> {
        require!(title.len() <= 64, QuestError::StringTooLong);

        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.verifier_keeper = verifier_keeper;
        config.hackathon_id = hackathon_id;
        config.quest_count = 0;
        config.total_bounties_distributed = 0;
        config.vault_bump = ctx.bumps.vault;
        config.bump = ctx.bumps.config;

        emit!(HackathonInitialized {
            hackathon_id,
            authority: config.authority,
            verifier_keeper,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Organizers register distinct technical milestone tracks (e.g., Core Anchor, Token Extensions).
    pub fn create_quest(
        ctx: Context<CreateQuest>,
        quest_id: u8,
        reward_amount: u64,
        xp_points: u32,
        max_completions: u32,
        metadata_hash: [u8; 32],
    ) -> Result<()> {
        require!(quest_id < MAX_QUESTS as u8, QuestError::InvalidQuestId);
        require!(reward_amount > 0, QuestError::InvalidRewardAmount);

        let quest = &mut ctx.accounts.quest;
        quest.quest_id = quest_id;
        quest.reward_amount = reward_amount;
        quest.xp_points = xp_points;
        quest.max_completions = max_completions;
        quest.current_completions = 0;
        quest.metadata_hash = metadata_hash;
        quest.is_active = true;
        quest.bump = ctx.bumps.quest;

        let config = &mut ctx.accounts.config;
        config.quest_count = config.quest_count.checked_add(1).ok_or(QuestError::MathOverflow)?;

        emit!(QuestCreated {
            hackathon_id: config.hackathon_id,
            quest_id,
            reward_amount,
            xp_points,
        });

        Ok(())
    }

    /// Builder onboards to the hackathon; allocates a personal bitmask tracking account.
    pub fn register_builder(ctx: Context<RegisterBuilder>) -> Result<()> {
        let builder = &mut ctx.accounts.builder;
        builder.builder = ctx.accounts.signer.key();
        builder.hackathon = ctx.accounts.config.key();
        builder.completed_mask = 0;
        builder.total_xp = 0;
        builder.registered_at = Clock::get()?.unix_timestamp;
        builder.bump = ctx.bumps.builder;

        emit!(BuilderRegistered {
            hackathon: builder.hackathon,
            builder: builder.builder,
            registered_at: builder.registered_at,
        });

        Ok(())
    }

    /// Keeper attests that builder completed verifiable on-chain requirements and disburses rewards.
    pub fn complete_quest_and_disburse(
        ctx: Context<CompleteQuestAndDisburse>,
        quest_id: u8,
        proof_signature: [u8; 64],
    ) -> Result<()> {
        let quest = &mut ctx.accounts.quest;
        let builder = &mut ctx.accounts.builder;
        let config = &mut ctx.accounts.config;

        require!(quest.is_active, QuestError::QuestInactive);
        require!(
            quest.current_completions < quest.max_completions,
            QuestError::QuestCapReached
        );

        // Bitwise completion validation (Sybil / Reentrancy protection)
        let quest_mask: u128 = 1u128 << quest_id;
        require!(
            (builder.completed_mask & quest_mask) == 0,
            QuestError::QuestAlreadyCompleted
        );

        // Check escrow vault balance
        let vault_balance = ctx.accounts.vault.to_account_info().lamports();
        require!(
            vault_balance >= quest.reward_amount,
            QuestError::InsufficientVaultBalance
        );

        // Update builder state
        builder.completed_mask |= quest_mask;
        builder.total_xp = builder
            .total_xp
            .checked_add(quest.xp_points as u64)
            .ok_or(QuestError::MathOverflow)?;

        // Update quest state
        quest.current_completions = quest
            .current_completions
            .checked_add(1)
            .ok_or(QuestError::MathOverflow)?;

        config.total_bounties_distributed = config
            .total_bounties_distributed
            .checked_add(quest.reward_amount)
            .ok_or(QuestError::MathOverflow)?;

        // PDA Cross-Program Invocation to transfer SOL bounty from vault to builder
        let hackathon_id_bytes = config.hackathon_id.to_le_bytes();
        let seeds: &[&[u8]] = &[
            VAULT_SEED,
            config.to_account_info().key.as_ref(),
            &[config.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let transfer_cpi = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.builder_recipient.to_account_info(),
            },
            signer_seeds,
        );
        transfer(transfer_cpi, quest.reward_amount)?;

        emit!(QuestCompleted {
            hackathon: config.key(),
            builder: builder.builder,
            quest_id,
            reward_distributed: quest.reward_amount,
            total_xp: builder.total_xp,
            proof_signature,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION STRUCTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(hackathon_id: u64)]
pub struct InitializeHackathon<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + HackathonConfig::INIT_SPACE,
        seeds = [CONFIG_SEED, hackathon_id.to_le_bytes().as_ref()],
        bump
    )]
    pub config: Account<'info, HackathonConfig>,

    /// CHECK: PDA escrow vault that securely holds reward SOL.
    #[account(
        mut,
        seeds = [VAULT_SEED, config.key().as_ref()],
        bump
    )]
    pub vault: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(quest_id: u8)]
pub struct CreateQuest<'info> {
    #[account(
        mut,
        has_one = authority @ QuestError::UnauthorizedAuthority
    )]
    pub config: Account<'info, HackathonConfig>,

    #[account(
        init,
        payer = authority,
        space = 8 + Quest::INIT_SPACE,
        seeds = [QUEST_SEED, config.key().as_ref(), &[quest_id]],
        bump
    )]
    pub quest: Account<'info, Quest>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterBuilder<'info> {
    #[account(mut)]
    pub config: Account<'info, HackathonConfig>,

    #[account(
        init,
        payer = signer,
        space = 8 + BuilderProfile::INIT_SPACE,
        seeds = [BUILDER_SEED, config.key().as_ref(), signer.key().as_ref()],
        bump
    )]
    pub builder: Account<'info, BuilderProfile>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(quest_id: u8)]
pub struct CompleteQuestAndDisburse<'info> {
    #[account(
        mut,
        has_one = verifier_keeper @ QuestError::UnauthorizedKeeper
    )]
    pub config: Account<'info, HackathonConfig>,

    #[account(
        mut,
        seeds = [QUEST_SEED, config.key().as_ref(), &[quest_id]],
        bump = quest.bump
    )]
    pub quest: Account<'info, Quest>,

    #[account(
        mut,
        seeds = [BUILDER_SEED, config.key().as_ref(), builder.builder.as_ref()],
        bump = builder.bump
    )]
    pub builder: Account<'info, BuilderProfile>,

    /// CHECK: Bound to builder.builder account address
    #[account(
        mut,
        address = builder.builder @ QuestError::InvalidRecipientAddress
    )]
    pub builder_recipient: AccountInfo<'info>,

    /// CHECK: PDA escrow vault that holds rewards
    #[account(
        mut,
        seeds = [VAULT_SEED, config.key().as_ref()],
        bump = config.vault_bump
    )]
    pub vault: AccountInfo<'info>,

    pub verifier_keeper: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// STATE STRUCTS
// -----------------------------------------------------------------------------

#[account]
pub struct HackathonConfig {
    pub authority: Pubkey,                  // 32
    pub verifier_keeper: Pubkey,            // 32
    pub hackathon_id: u64,                  // 8
    pub quest_count: u32,                   // 4
    pub total_bounties_distributed: u64,    // 8
    pub vault_bump: u8,                     // 1
    pub bump: u8,                           // 1
}

impl HackathonConfig {
    pub const INIT_SPACE: usize = 32 + 32 + 8 + 4 + 8 + 1 + 1;
}

#[account]
pub struct Quest {
    pub quest_id: u8,                       // 1
    pub reward_amount: u64,                 // 8
    pub xp_points: u32,                     // 4
    pub max_completions: u32,               // 4
    pub current_completions: u32,           // 4
    pub metadata_hash: [u8; 32],            // 32
    pub is_active: bool,                    // 1
    pub bump: u8,                           // 1
}

impl Quest {
    pub const INIT_SPACE: usize = 1 + 8 + 4 + 4 + 4 + 32 + 1 + 1;
}

#[account]
pub struct BuilderProfile {
    pub builder: Pubkey,                    // 32
    pub hackathon: Pubkey,                  // 32
    pub completed_mask: u128,               // 16 (supports up to 128 unique quests)
    pub total_xp: u64,                      // 8
    pub registered_at: i64,                 // 8
    pub bump: u8,                           // 1
}

impl BuilderProfile {
    pub const INIT_SPACE: usize = 32 + 32 + 16 + 8 + 8 + 1;
}

// -----------------------------------------------------------------------------
// EVENTS & ERRORS
// -----------------------------------------------------------------------------

#[event]
pub struct HackathonInitialized {
    pub hackathon_id: u64,
    pub authority: Pubkey,
    pub verifier_keeper: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct QuestCreated {
    pub hackathon_id: u64,
    pub quest_id: u8,
    pub reward_amount: u64,
    pub xp_points: u32,
}

#[event]
pub struct BuilderRegistered {
    pub hackathon: Pubkey,
    pub builder: Pubkey,
    pub registered_at: i64,
}

#[event]
pub struct QuestCompleted {
    pub hackathon: Pubkey,
    pub builder: Pubkey,
    pub quest_id: u8,
    pub reward_distributed: u64,
    pub total_xp: u64,
    pub proof_signature: [u8; 64],
}

#[error_code]
pub enum QuestError {
    #[msg("String exceeds maximum allowed capacity")]
    StringTooLong,
    #[msg("Quest ID exceeds maximum threshold (128)")]
    InvalidQuestId,
    #[msg("Reward amount must be greater than zero")]
    InvalidRewardAmount,
    #[msg("Signer is not authorized hackathon authority")]
    UnauthorizedAuthority,
    #[msg("Signer is not the authorized verification keeper")]
    UnauthorizedKeeper,
    #[msg("Builder recipient does not match profile authority")]
    InvalidRecipientAddress,
    #[msg("Quest is not active")]
    QuestInactive,
    #[msg("Maximum completions for this quest reached")]
    QuestCapReached,
    #[msg("Quest milestone already satisfied by this builder")]
    QuestAlreadyCompleted,
    #[msg("Escrow vault possesses insufficient lamports for bounty disburse")]
    InsufficientVaultBalance,
    #[msg("Arithmetic overflow detected")]
    MathOverflow,
}