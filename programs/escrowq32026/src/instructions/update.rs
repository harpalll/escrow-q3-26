use anchor_lang::prelude::*;

use crate::{state::Escrow, EscrowError, ESCROW_SEED};

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,

    #[account(
        mut,
        has_one = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,

    pub clock: Sysvar<'info, Clock>,
}

impl<'info> Update<'info> {
    pub fn update(&mut self, expiration: i64) -> Result<()> {
        require!(
            self.escrow.expiration > self.clock.unix_timestamp,
            EscrowError::EscrowExpired
        );

        require!(
            expiration > self.clock.unix_timestamp,
            EscrowError::InvalidExpiration
        );

        self.escrow.expiration = expiration;

        Ok(())
    }
}
