**ctx.accounts.escrow_vault.to_account_info().try_borrow_mut_lamports()? -= payout;
   **ctx.accounts.builder.to_account_info().try_borrow_mut_lamports()? += payout;