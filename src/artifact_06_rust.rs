// File: /programs/builder_ignition/src/state.rs
use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct HackathonConfig {
    /// Authority who configured the hackathon (DAO / Organizer).
    pub organizer: Pubkey,
    /// Mint address of the reward token (e.g., USDC).
    pub reward_mint: Pubkey,
    /// Canonical bump for the HackathonConfig PDA.
    pub bump: u8,
    /// Canonical bump for the Escrow Token Account PDA.
    pub vault_bump: u8,
    /// Fixed reward payout per validated milestone (in base token units).
    pub milestone_reward_amount: u64,
    /// Total number of builders enrolled.
    pub total_enrolled: u32,
    /// Total rewards disbursed across all builders.
    pub total_disbursed: u64,
    /// Active state flag.
    pub is_active: bool,
}

#[account]
#[derive(InitSpace)]
pub struct BuilderRecord {
    /// The wallet of the participating developer.
    pub builder: Pubkey,
    /// Timestamp of enrollment.
    pub enrolled_at: i64,
    /// IPFS / Arweave Content Hash for the project submission (SHA-256 fixed bytes).
    pub submission_hash: [u8; 32],
    /// Milestone status: 0 = Unsubmitted, 1 = Submitted, 2 = Verified & Disbursed.
    pub status: u8,
    /// PDA bump for the builder record.
    pub bump: u8,
}

pub const STATUS_UNSUBMITTED: u8 = 0;
pub const STATUS_SUBMITTED: u8 = 1;
pub const STATUS_VERIFIED: u8 = 2;