use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("BQst111111111111111111111111111111111111111");

pub const CONFIG_SEED: &[u8] = b"config";
pub const TRACK_SEED: &[u8] = b"track";
pub const BUILDER_SEED: &[u8] = b"builder";
pub const VAULT_SEED: &[u8] = b"vault";

#[program]
pub mod builder_quest {
    use super::*;

    /// Initializes the root protocol configuration.
    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        verification_oracle: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.verification_oracle = verification_oracle;
        config.total_tracks = 0;
        config.total_builders = 0;
        config.bump = ctx.bumps.config;

        emit!(ProtocolInitializedEvent {
            authority: config.authority,
            oracle: verification_oracle,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Sponsors create a track with fixed-allocation prizes.
    pub fn create_track(
        ctx: Context<CreateTrack>,
        track_id: u64,
        reward_per_builder: u64,
        max_builders: u32,
        metadata_hash: [u8; 32],
    ) -> Result<()> {
        require!(reward_per_builder > 0, QuestError::InvalidRewardAmount);
        require!(max_builders > 0, QuestError::ZeroMaxBuilders);

        let track = &mut ctx.accounts.track;
        track.track_id = track_id;
        track.sponsor = ctx.accounts.sponsor.key();
        track.reward_per_builder = reward_per_builder;
        track.max_builders = max_builders;
        track.completed_builders = 0;
        track.metadata_hash = metadata_hash;
        track.is_active = true;
        track.bump = ctx.bumps.track;
        track.vault_bump = ctx.bumps.escrow_vault;

        let config = &mut ctx.accounts.config;
        config.total_tracks = config
            .total_tracks
            .checked_add(1)
            .ok_or(QuestError::ArithmeticOverflow)?;

        emit!(TrackCreatedEvent {
            track_id,
            sponsor: track.sponsor,
            reward_per_builder,
            max_builders,
        });

        Ok(())
    }

    /// Deposits rewards into the track escrow vault.
    pub fn fund_track_escrow(ctx: Context<FundTrackEscrow>, amount: u64) -> Result<()> {
        require!(amount > 0, QuestError::InvalidDepositAmount);

        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.funder.to_account_info(),
                to: ctx.accounts.escrow_vault.to_account_info(),
            },
        );
        transfer(cpi_context, amount)?;

        emit!(EscrowFundedEvent {
            track_id: ctx.accounts.track.track_id,
            funder: ctx.accounts.funder.key(),
            amount,
        });

        Ok(())
    }

    /// Initializes a builder profile PDA for progress tracking.
    pub fn register_builder(ctx: Context<RegisterBuilder>, github_hash: [u8; 32]) -> Result<()> {
        let profile = &mut ctx.accounts.builder_profile;
        profile.builder = ctx.accounts.builder.key();
        profile.github_hash = github_hash;
        profile.completed_mask = 0;
        profile.reputation_points = 0;
        profile.bump = ctx.bumps.builder_profile;

        let config = &mut ctx.accounts.config;
        config.total_builders = config
            .total_builders
            .checked_add(1)
            .ok_or(QuestError::ArithmeticOverflow)?;

        emit!(BuilderRegisteredEvent {
            builder: profile.builder,
            github_hash,
        });

        Ok(())
    }

    /// Verifies track completion and disburses grant reward to builder.
    pub fn complete_track_milestone(
        ctx: Context<CompleteTrackMilestone>,
        track_id: u64,
        solution_hash: [u8; 32],
    ) -> Result<()> {
        let track = &mut ctx.accounts.track;
        let profile = &mut ctx.accounts.builder_profile;

        require!(track.is_active, QuestError::TrackInactive);
        require!(
            track.completed_builders < track.max_builders,
            QuestError::TrackCapReached
        );

        // Verification bounds checking (tracks 0..63 mapped via bitmask)
        require!(track_id < 64, QuestError::InvalidTrackId);
        let mask: u64 = 1u64
            .checked_shl(track_id as u32)
            .ok_or(QuestError::ArithmeticOverflow)?;
        require!(
            (profile.completed_mask & mask) == 0,
            QuestError::TrackAlreadyCompleted
        );

        // Verify Vault Balance
        let vault_lamports = ctx.accounts.escrow_vault.lamports();
        let rent_exempt_minimum = Rent::get()?.minimum_balance(0);
        let available_balance = vault_lamports
            .checked_sub(rent_exempt_minimum)
            .ok_or(QuestError::InsufficientVaultBalance)?;

        require!(
            available_balance >= track.reward_per_builder,
            QuestError::InsufficientVaultBalance
        );

        // Update State
        profile.completed_mask |= mask;
        profile.reputation_points = profile
            .reputation_points
            .checked_add(100)
            .ok_or(QuestError::ArithmeticOverflow)?;

        track.completed_builders = track
            .completed_builders
            .checked_add(1)
            .ok_or(QuestError::ArithmeticOverflow)?;

        // Payout reward via Escrow PDA Transfer
        let track_id_bytes = track_id.to_le_bytes();
        let seeds = &[
            VAULT_SEED,
            track_id_bytes.as_ref(),
            &[ctx.accounts.track.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        **ctx
            .accounts
            .escrow_vault
            .to_account_info()
            .try_borrow_mut_lamports()? = vault_lamports
            .checked_sub(track.reward_per_builder)
            .ok_or(QuestError::ArithmeticOverflow)?;

        **ctx
            .accounts
            .builder
            .to_account_info()
            .try_borrow_mut_lamports()? = ctx
            .accounts
            .builder
            .lamports()
            .checked_add(track.reward_per_builder)
            .ok_or(QuestError::ArithmeticOverflow)?;

        emit!(MilestoneCompletedEvent {
            builder: profile.builder,
            track_id,
            solution_hash,
            payout_lamports: track.reward_per_builder,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION STRUCTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + ProtocolConfig::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, ProtocolConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(track_id: u64)]
pub struct CreateTrack<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = authority @ QuestError::UnauthorizedAuthority
    )]
    pub config: Account<'info, ProtocolConfig>,
    pub authority: Signer<'info>,
    #[account(mut)]
    pub sponsor: Signer<'info>,
    #[account(
        init,
        payer = sponsor,
        space = 8 + TrackAccount::INIT_SPACE,
        seeds = [TRACK_SEED, track_id.to_le_bytes().as_ref()],
        bump
    )]
    pub track: Account<'info, TrackAccount>,
    /// CHECK: PDA escrow vault to store reward lamports
    #[account(
        mut,
        seeds = [VAULT_SEED, track_id.to_le_bytes().as_ref()],
        bump
    )]
    pub escrow_vault: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(track_id: u64)]
pub struct FundTrackEscrow<'info> {
    #[account(
        seeds = [TRACK_SEED, track_id.to_le_bytes().as_ref()],
        bump = track.bump,
    )]
    pub track: Account<'info, TrackAccount>,
    /// CHECK: Target PDA verified by seed derivation
    #[account(
        mut,
        seeds = [VAULT_SEED, track_id.to_le_bytes().as_ref()],
        bump = track.vault_bump
    )]
    pub escrow_vault: AccountInfo<'info>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterBuilder<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, ProtocolConfig>,
    #[account(mut)]
    pub builder: Signer<'info>,
    #[account(
        init,
        payer = builder,
        space = 8 + BuilderProfile::INIT_SPACE,
        seeds = [BUILDER_SEED, builder.key().as_ref()],
        bump
    )]
    pub builder_profile: Account<'info, BuilderProfile>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(track_id: u64)]
pub struct CompleteTrackMilestone<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfig>,
    #[account(
        mut,
        seeds = [TRACK_SEED, track_id.to_le_bytes().as_ref()],
        bump = track.bump
    )]
    pub track: Account<'info, TrackAccount>,
    /// CHECK: Escrow account validating seed and bump
    #[account(
        mut,
        seeds = [VAULT_SEED, track_id.to_le_bytes().as_ref()],
        bump = track.vault_bump
    )]
    pub escrow_vault: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [BUILDER_SEED, builder.key().as_ref()],
        bump = builder_profile.bump,
        has_one = builder @ QuestError::UnauthorizedBuilder
    )]
    pub builder_profile: Account<'info, BuilderProfile>,
    #[account(mut)]
    pub builder: Signer<'info>,
    #[account(
        constraint = oracle.key() == config.verification_oracle @ QuestError::InvalidOracleSigner
    )]
    pub oracle: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// STATE SCHEMAS
// -----------------------------------------------------------------------------

#[account]
#[derive(InitSpace)]
pub struct ProtocolConfig {
    pub authority: Pubkey,
    pub verification_oracle: Pubkey,
    pub total_tracks: u64,
    pub total_builders: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct TrackAccount {
    pub track_id: u64,
    pub sponsor: Pubkey,
    pub reward_per_builder: u64,
    pub max_builders: u32,
    pub completed_builders: u32,
    pub metadata_hash: [u8; 32],
    pub is_active: bool,
    pub bump: u8,
    pub vault_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct BuilderProfile {
    pub builder: Pubkey,
    pub github_hash: [u8; 32],
    pub completed_mask: u64,
    pub reputation_points: u32,
    pub bump: u8,
}

// -----------------------------------------------------------------------------
// EVENTS & ERRORS
// -----------------------------------------------------------------------------

#[event]
pub struct ProtocolInitializedEvent {
    pub authority: Pubkey,
    pub oracle: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TrackCreatedEvent {
    pub track_id: u64,
    pub sponsor: Pubkey,
    pub reward_per_builder: u64,
    pub max_builders: u32,
}

#[event]
pub struct EscrowFundedEvent {
    pub track_id: u64,
    pub funder: Pubkey,
    pub amount: u64,
}

#[event]
pub struct BuilderRegisteredEvent {
    pub builder: Pubkey,
    pub github_hash: [u8; 32],
}

#[event]
pub struct MilestoneCompletedEvent {
    pub builder: Pubkey,
    pub track_id: u64,
    pub solution_hash: [u8; 32],
    pub payout_lamports: u64,
}

#[error_code]
pub enum QuestError {
    #[msg("Invalid reward amount specified.")]
    InvalidRewardAmount,
    #[msg("Max builders must be greater than zero.")]
    ZeroMaxBuilders,
    #[msg("Escrow deposit must be greater than zero.")]
    InvalidDepositAmount,
    #[msg("Track is currently inactive.")]
    TrackInactive,
    #[msg("Maximum builder quota reached for this track.")]
    TrackCapReached,
    #[msg("Track ID must be within index range [0..63].")]
    InvalidTrackId,
    #[msg("Milestone for this track has already been completed by the builder.")]
    TrackAlreadyCompleted,
    #[msg("Insufficient unreserved lamports in track escrow vault.")]
    InsufficientVaultBalance,
    #[msg("Signer is not authorized to act as oracle.")]
    InvalidOracleSigner,
    #[msg("Signer is not authorized for config authority.")]
    UnauthorizedAuthority,
    #[msg("Signer does not match builder profile.")]
    UnauthorizedBuilder,
    #[msg("Arithmetic operation resulted in an overflow.")]
    ArithmeticOverflow,
}