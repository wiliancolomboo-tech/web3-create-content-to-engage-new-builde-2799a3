use anchor_lang::prelude::*;

pub const PROTOCOL_CONFIG_SEED: &[u8] = b"protocol_config";
pub const QUEST_SEED: &[u8] = b"quest";
pub const VAULT_SEED: &[u8] = b"quest_vault";
pub const PROGRESS_SEED: &[u8] = b"builder_progress";

#[account]
pub struct ProtocolConfig {
    pub admin: Pubkey,          // 32
    pub fee_relayer: Pubkey,    // 32
    pub total_quests: u64,      // 8
    pub is_paused: bool,        // 1
    pub bump: u8,               // 1
}

impl ProtocolConfig {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 1 + 1; // 82 bytes
}

#[account]
pub struct Quest {
    pub quest_id: u64,          // 8
    pub creator: Pubkey,        // 32
    pub expected_hash: [u8; 32],// 32 (SHA-256 target of code answer)
    pub reward_amount: u64,     // 8 (SOL in lamports)
    pub expires_at: i64,        // 8
    pub max_completions: u32,   // 4
    pub current_completions: u32,// 4
    pub is_active: bool,        // 1
    pub bump: u8,               // 1
    pub vault_bump: u8,         // 1
}

impl Quest {
    pub const LEN: usize = 8 + 8 + 32 + 32 + 8 + 8 + 4 + 4 + 1 + 1 + 1; // 107 bytes
}

#[account]
pub struct BuilderProgress {
    pub builder: Pubkey,        // 32
    pub quest_id: u64,          // 8
    pub completed_at: i64,      // 8
    pub reward_claimed: bool,   // 1
    pub execution_score: u16,   // 2
    pub bump: u8,               // 1
}

impl BuilderProgress {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 1 + 2 + 1; // 60 bytes
}