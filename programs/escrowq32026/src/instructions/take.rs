use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

use crate::{state::Escrow, EscrowError, ESCROW_SEED};

#[derive(Accounts)]
pub struct Take<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::authority = taker,
        associated_token::mint = mint_a,
        associated_token::token_program = token_program
    )]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = taker,
        associated_token::token_program = token_program
    )]
    pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub maker: SystemAccount<'info>,

    #[account(
        init_if_needed,
        payer = taker,
        associated_token::mint = mint_b,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    pub mint_a: InterfaceAccount<'info, Mint>,
    pub mint_b: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        close = maker,
        has_one = mint_a,
        has_one = mint_b,
        has_one = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump
    )]
    pub escrow: Box<Account<'info, Escrow>>,

    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Take<'info> {
    pub fn take(&mut self) -> Result<()> {
        let seed_bytes = self.escrow.seed.to_le_bytes();
        let bump_bytes = [self.escrow.bump];

        let signer_seeds: &[&[&[u8]]] = &[&[
            ESCROW_SEED,
            self.maker.key.as_ref(),
            &seed_bytes,
            &bump_bytes,
        ]];

        let cpi_program = self.token_program.key();

        // Transfer mint_a from vault → taker (PDA signs)
        let cpi_accounts = TransferChecked {
            authority: self.escrow.to_account_info(),
            from: self.vault.to_account_info(),
            to: self.taker_ata_a.to_account_info(),
            mint: self.mint_a.to_account_info(),
        };

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        transfer_checked(cpi_context, self.vault.amount, self.mint_a.decimals)?;

        // Transfer mint_b from taker → maker (taker signs)
        let cpi_accounts = TransferChecked {
            authority: self.taker.to_account_info(),
            from: self.taker_ata_b.to_account_info(),
            to: self.maker_ata_b.to_account_info(),
            mint: self.mint_b.to_account_info(),
        };

        let cpi_context = CpiContext::new(cpi_program, cpi_accounts);
        transfer_checked(cpi_context, self.escrow.receive, self.mint_b.decimals)?;

        // Close vault → rent returned to maker (PDA signs)
        let cpi_accounts = CloseAccount {
            account: self.vault.to_account_info(),
            destination: self.maker.to_account_info(),
            authority: self.escrow.to_account_info(),
        };

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        close_account(cpi_context)?;

        Ok(())
    }
}
