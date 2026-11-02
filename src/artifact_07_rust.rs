// File: /programs/builder_ignition/src/errors.rs
use anchor_lang::prelude::*;

#[error_code]
pub enum IgnitionError {
    #[msg("The hackathon campaign is currently inactive.")]
    HackathonInactive,
    #[msg("Provided milestone status transition is invalid.")]
    InvalidStatusTransition,
    #[msg("Milestone submission has already been verified and disbursed.")]
    AlreadyDisbursed,
    #[msg("Submission hash cannot be empty or zeroed.")]
    EmptySubmissionHash,
    #[msg("Escrow token vault balance is insufficient to process reward payout.")]
    InsufficientVaultBalance,
    #[msg("Arithmetic overflow occurred during balance calculation.")]
    NumericalOverflow,
    #[msg("Unauthorized keeper or evaluator.")]
    UnauthorizedEvaluator,
}