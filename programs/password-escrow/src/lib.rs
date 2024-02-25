pub mod error;

use crate::error::PasswordEscrowError;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token_interface::{
    CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};

declare_id!("FZrLq9ehNww5jnGkZ3suXiwUGJtg6socYadSPWYhFgUS");

#[program]
pub mod password_escrow {
    use super::*;
    use anchor_spl::token_interface;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        config_authority: Pubkey,
        withdraw_authority: Pubkey,
        fee_bps: u64,
    ) -> Result<()> {
        // TODO validate fee_bps

        let config = &mut ctx.accounts.config;
        config.config_authority = config_authority;
        config.withdraw_authority = withdraw_authority;
        config.fee_bps = fee_bps;
        config.signer = ctx.accounts.signer.key();
        config.signer_bump = ctx.bumps.signer;

        Ok(())
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        config_authority: Pubkey,
        withdraw_authority: Pubkey,
        fee_bps: u64,
    ) -> Result<()> {
        // TODO validate fee_bps

        let config = &mut ctx.accounts.config;
        config.config_authority = config_authority;
        config.withdraw_authority = withdraw_authority;
        config.fee_bps = fee_bps;

        Ok(())
    }

    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        Ok(())
    }

    pub fn deposit(
        ctx: Context<Deposit>,
        amount: u64,
        salt: [u8; 16],
        beneficiary: Pubkey,
    ) -> Result<()> {
        require_gt!(amount, 0, PasswordEscrowError::ZeroAmount);

        let decimals = ctx.accounts.mint.decimals;
        token_interface::transfer_checked(ctx.accounts.into(), amount, decimals)?;

        let escrow = &mut ctx.accounts.escrow;
        escrow.config = ctx.accounts.config.key();
        escrow.depositor = ctx.accounts.depositor.key();
        escrow.beneficiary = beneficiary;
        escrow.salt = salt;
        escrow.mint = ctx.accounts.mint.key();
        escrow.vault = ctx.accounts.vault.key();
        escrow.escrow_bump = ctx.bumps.escrow;

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>) -> Result<()> {
        let beneficiary = ctx.accounts.beneficiary.key();
        let config = ctx.accounts.config.key();

        let singer_seeds = [
            b"escrow".as_ref(),
            beneficiary.as_ref(),
            config.as_ref(),
            &[ctx.accounts.escrow.escrow_bump],
        ];

        let decimals = ctx.accounts.mint.decimals;
        let amount = ctx.accounts.vault.amount;
        let cpi_ctx: CpiContext<_> = ctx.accounts.into();
        token_interface::transfer_checked(cpi_ctx.with_signer(&[&singer_seeds]), amount, decimals)?;

        let cpi_ctx: CpiContext<_> = ctx.accounts.into();
        token_interface::close_account(cpi_ctx.with_signer(&[&singer_seeds]))
    }
}

#[account]
#[derive(InitSpace)]
pub struct EscrowConfig {
    pub config_authority: Pubkey,
    pub withdraw_authority: Pubkey,
    pub signer: Pubkey,
    pub fee_bps: u64,
    pub signer_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub config: Pubkey,
    pub depositor: Pubkey,
    pub beneficiary: Pubkey,
    pub salt: [u8; 16],

    pub mint: Pubkey,
    pub vault: Pubkey,

    pub escrow_bump: u8,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(init, payer = payer, space = 8 + EscrowConfig::INIT_SPACE)]
    pub config: Account<'info, EscrowConfig>,

    /// CHECK: Only for bump calculation
    #[account(seeds = [b"signer".as_ref(), config.key().as_ref()], bump)]
    pub signer: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(mut, has_one = config_authority)]
    pub config: Account<'info, EscrowConfig>,
    pub config_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(has_one = withdraw_authority)]
    pub config: Account<'info, EscrowConfig>,
    pub withdraw_authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(amount: u64, salt: [u8; 16], beneficiary: Pubkey)]
pub struct Deposit<'info> {
    pub config: Account<'info, EscrowConfig>,
    #[account(
        init,
        payer = depositor,
        seeds = [
            b"escrow",
            beneficiary.as_ref(),
            config.key().as_ref(),
        ],
        bump,
        space = 8 + Escrow::INIT_SPACE
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(mut,
        token::mint = mint,
        token::authority = depositor,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(init,
        payer = depositor,
        associated_token::mint = mint,
        associated_token::authority = escrow,
        associated_token::token_program = token_program,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'a, 'b, 'c, 'info> From<&mut Deposit<'info>>
    for CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>>
{
    fn from(
        accounts: &mut Deposit<'info>,
    ) -> CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: accounts.token_account.to_account_info(),
            mint: accounts.mint.to_account_info(),
            to: accounts.vault.to_account_info(),
            authority: accounts.depositor.to_account_info(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub config: Account<'info, EscrowConfig>,

    #[account(
        mut,
        close = config,
        has_one = config,
        has_one = beneficiary,
        has_one = vault,
        seeds = [
            b"escrow",
            beneficiary.key().as_ref(),
            config.key().as_ref(),
        ],
        bump = escrow.escrow_bump,
    )]
    pub escrow: Account<'info, Escrow>,
    pub beneficiary: Signer<'info>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = escrow,
        token::token_program = token_program,
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = mint,
        token::token_program = token_program,
    )]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'a, 'b, 'c, 'info> From<&mut Withdraw<'info>>
    for CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>>
{
    fn from(
        accounts: &mut Withdraw<'info>,
    ) -> CpiContext<'a, 'b, 'c, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: accounts.vault.to_account_info(),
            mint: accounts.mint.to_account_info(),
            to: accounts.destination.to_account_info(),
            authority: accounts.escrow.to_account_info(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'a, 'b, 'c, 'info> From<&mut Withdraw<'info>>
    for CpiContext<'a, 'b, 'c, 'info, CloseAccount<'info>>
{
    fn from(accounts: &mut Withdraw<'info>) -> CpiContext<'a, 'b, 'c, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: accounts.vault.to_account_info(),
            destination: accounts.config.to_account_info(),
            authority: accounts.escrow.to_account_info(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}
