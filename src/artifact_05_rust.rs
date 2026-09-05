use anchor_lang::prelude::*;

#[error_code]
pub enum SolQuestError {
    #[msg("Protocol is currently paused by admin.")]
    ProtocolPaused,
    #[msg("Quest has expired or is not yet active.")]
    QuestInactive,
    #[msg("Invalid cryptographic solution hash preimage.")]
    InvalidSolutionPreimage,
    #[msg("Quest has already been completed by this builder.")]
    QuestAlreadyCompleted,
    #[msg("Quest milestone not eligible for payout.")]
    QuestNotEligibleForPayout,
    #[msg("Reward escrow vault has insufficient funds.")]
    InsufficientVaultBalance,
    #[msg("Unauthorized protocol administrator.")]
    UnauthorizedAdmin,
    #[msg("String length exceeds maximal permitted size.")]
    StringExceedsLimit,
    #[msg("Arithmetic overflow detected during state transition.")]
    ArithmeticOverflow,
}