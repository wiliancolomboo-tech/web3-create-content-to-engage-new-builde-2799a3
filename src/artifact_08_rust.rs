// File: /programs/builder_ignition/src/lib.rs
pub mod errors;
pub mod state;

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use crate::errors::IgnitionError;
use crate::state::*;

declare_id!("Ignit111111111111111111111111111111111111111");

#[program]
pub mod builder_ignition {
    use super::*;

    /// Initializes a hackathon escrow workspace.
    pub fn initialize_hackathon(
        ctx: Context<InitializeHackathon>,
        milestone_reward_amount: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.hackathon_config;
        config.organizer = ctx.accounts.organizer.key();
        config.reward_mint = ctx.accounts.reward_mint.key();
        config.bump = ctx.bumps.hackathon_config;
        config.vault_bump = ctx.bumps.escrow_vault;
        config.milestone_reward_amount = milestone_reward_amount;
        config.total_enrolled = 0;
        config.total_disbursed = 0;
        config.is_active = true;

        msg!("Hackathon initialized. Organizer: {}", config.organizer);
        Ok(())
    }

    /// Registers a new builder for the hackathon by provisioning a BuilderRecord PDA.
    pub fn register_builder(ctx: Context<RegisterBuilder>) -> Result<()> {
        let config = &mut ctx.accounts.hackathon_config;
        require!(config.is_active, IgnitionError::HackathonInactive);

        let builder_record = &mut ctx.accounts.builder_record;
        builder_record.builder = ctx.accounts.builder.key();
        builder_record.enrolled_at = Clock::get()?.unix_timestamp;
        builder_record.submission_hash = [0u8; 32];
        builder_record.status = STATUS_UNSUBMITTED;
        builder_record.bump = ctx.bumps.builder_record;

        config.total_enrolled = config
            .total_enrolled
            .checked_add(1)
            .ok_or(IgnitionError::NumericalOverflow)?;

        msg!("Builder registered: {}", builder_record.builder);
        Ok(())
    }

    /// Builder submits milestone proof hash (e.g. SHA-256 of commit or Arweave CID).
    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        submission_hash: [u8; 32],
    ) -> Result<()> {
        require!(
            submission_hash != [0u8; 32],
            IgnitionError::EmptySubmissionHash
        );

        let builder_record = &mut ctx.accounts.builder_record;
        require!(
            builder_record.status != STATUS_VERIFIED,
            IgnitionError::AlreadyDisbursed
        );

        builder_record.submission_hash = submission_hash;
        builder_record.status = STATUS_SUBMITTED;

        msg!(
            "Milestone submitted by {}. Status: SUBMITTED",
            builder_record.builder
        );
        Ok(())
    }

    /// Evaluator/Organizer validates milestone and triggers CPI token payout from PDA Escrow.
    pub fn verify_and_disburse(ctx: Context<VerifyAndDisburse>) -> Result<()> {
        let config = &mut ctx.accounts.hackathon_config;
        let builder_record = &mut ctx.accounts.builder_record;

        require!(config.is_active, IgnitionError::HackathonInactive);
        require!(
            builder_record.status == STATUS_SUBMITTED,
            IgnitionError::InvalidStatusTransition
        );
        require!(
            ctx.accounts.escrow_vault.amount >= config.milestone_reward_amount,
            IgnitionError::InsufficientVaultBalance
        );

        // CPI Signer seeds construction for Escrow PDA
        let organizer_key = config.organizer;
        let seeds = &[
            b"hackathon".as_ref(),
            organizer_key.as_ref(),
            &[config.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        // Execute CPI Token Transfer
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.builder_token_account.to_account_info(),
            authority: config.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        token::transfer(cpi_ctx, config.milestone_reward_amount)?;

        // Update state
        builder_record.status = STATUS_VERIFIED;
        config.total_disbursed = config
            .total_disbursed
            .checked_add(config.milestone_reward_amount)
            .ok_or(IgnitionError::NumericalOverflow)?;

        msg!(
            "Milestone verified. Disbursed {} tokens to {}",
            config.milestone_reward_amount,
            builder_record.builder
        );
        Ok(())
    }

    /// Sweeps remaining funds and closes the Hackathon PDA upon campaign completion.
    pub fn close_hackathon(ctx: Context<CloseHackathon>) -> Result<()> {
        let config = &ctx.accounts.hackathon_config;

        if ctx.accounts.escrow_vault.amount > 0 {
            let organizer_key = config.organizer;
            let seeds = &[
                b"hackathon".as_ref(),
                organizer_key.as_ref(),
                &[config.bump],
            ];
            let signer_seeds = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.organizer_token_account.to_account_info(),
                authority: config.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

            token::transfer(cpi_ctx, ctx.accounts.escrow_vault.amount)?;
        }

        msg!("Hackathon closed. Remaining funds swept to organizer.");
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION CONTEXTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeHackathon<'info> {
    #[account(
        init,
        payer = organizer,
        space = 8 + HackathonConfig::INIT_SPACE,
        seeds = [b"hackathon", organizer.key().as_ref()],
        bump
    )]
    pub hackathon_config: Account<'info, HackathonConfig>,

    pub reward_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = organizer,
        seeds = [b"escrow", hackathon_config.key().as_ref()],
        bump,
        token::mint = reward_mint,
        token::authority = hackathon_config
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub organizer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RegisterBuilder<'info> {
    #[account(
        mut,
        seeds = [b"hackathon", hackathon_config.organizer.as_ref()],
        bump = hackathon_config.bump
    )]
    pub hackathon_config: Account<'info, HackathonConfig>,

    #[account(
        init,
        payer = builder,
        space = 8 + BuilderRecord::INIT_SPACE,
        seeds = [b"builder", hackathon_config.key().as_ref(), builder.key().as_ref()],
        bump
    )]
    pub builder_record: Account<'info, BuilderRecord>,

    #[account(mut)]
    pub builder: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitMilestone<'info> {
    #[account(
        seeds = [b"hackathon", hackathon_config.organizer.as_ref()],
        bump = hackathon_config.bump
    )]
    pub hackathon_config: Account<'info, HackathonConfig>,

    #[account(
        mut,
        seeds = [b"builder", hackathon_config.key().as_ref(), builder.key().as_ref()],
        bump = builder_record.bump,
        has_one = builder
    )]
    pub builder_record: Account<'info, BuilderRecord>,

    pub builder: Signer<'info>,
}

#[derive(Accounts)]
pub struct VerifyAndDisburse<'info> {
    #[account(
        mut,
        seeds = [b"hackathon", hackathon_config.organizer.as_ref()],
        bump = hackathon_config.bump,
        has_one = organizer
    )]
    pub hackathon_config: Account<'info, HackathonConfig>,

    #[account(
        mut,
        seeds = [b"builder", hackathon_config.key().as_ref(), builder_record.builder.as_ref()],
        bump = builder_record.bump
    )]
    pub builder_record: Account<'info, BuilderRecord>,

    #[account(
        mut,
        seeds = [b"escrow", hackathon_config.key().as_ref()],
        bump = hackathon_config.vault_bump
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = builder_token_account.owner == builder_record.builder,
        constraint = builder_token_account.mint == hackathon_config.reward_mint
    )]
    pub builder_token_account: Account<'info, TokenAccount>,

    pub organizer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CloseHackathon<'info> {
    #[account(
        mut,
        seeds = [b"hackathon", organizer.key().as_ref()],
        bump = hackathon_config.bump,
        has_one = organizer,
        close = organizer
    )]
    pub hackathon_config: Account<'info, HackathonConfig>,

    #[account(
        mut,
        seeds = [b"escrow", hackathon_config.key().as_ref()],
        bump = hackathon_config.vault_bump
    )]
    pub escrow_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = organizer_token_account.owner == organizer.key(),
        constraint = organizer_token_account.mint == hackathon_config.reward_mint
    )]
    pub organizer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub organizer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}