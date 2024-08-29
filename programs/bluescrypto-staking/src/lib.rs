use solana_program::*;
use anchor_lang::prelude::*;
use anchor_lang::prelude::{ declare_id, borsh };
use solana_program::clock::Clock;
use std::mem::size_of;
use anchor_spl::token::{self, Mint, TokenAccount, Token, Transfer};

declare_id!("GWNpsKtdNy9LEZ4P86VHXGB8d25voFYKQDyvVNMixPcP");

#[program]
pub mod bluescrypto_staking {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let package_a = Package {
            name: String::from("BLUES Pebble Pounch"),
            max_deposit_amount: 100000000 * 1_000_000_000,
            total_locked_amount: 0,
            apr: 20,
            period: 60 * 60 * 24 * 30,
            percentage: 164
            // period: 60 * 3
        };

        let package_b = Package {
            name: String::from("Blue Wheel Guild"),
            max_deposit_amount: 75000000 * 1_000_000_000,
            total_locked_amount: 0,
            apr: 30,
            period: 60 * 60 * 24 * 30 * 2,
            percentage: 493
            // period: 60 * 6
        };

        let package_c = Package {
            name: String::from("Burrower's Bounty"),
            max_deposit_amount: 50000000 * 1_000_000_000,
            total_locked_amount: 0,
            apr: 45,
            period: 60 * 60 * 24 * 30 * 3,
            percentage: 1109
            // period: 60 * 9
        };

        let staking_storage = &mut ctx.accounts.staking_storage;
        staking_storage.packages.push(package_a);
        staking_storage.packages.push(package_b);
        staking_storage.packages.push(package_c);

        staking_storage.maintainer = *ctx.accounts.signer.key;

        Ok(())
    }

    pub fn stake(ctx: Context<Deposit>, package_index: u8, deposit_amount: u64) -> Result<()> {
        let staking_storage: &mut Account<StakingStorage> = &mut ctx.accounts.staking_storage;
        let packages = & staking_storage.packages;
        
        // validate package index
        if package_index >= packages.len() as u8{
            return Err(ErrorCode::InvalidPackageIndex.into());
        }

        let package = & packages[package_index as usize];

        // check if deposit amount is valid
        if (package.total_locked_amount + deposit_amount) > package.max_deposit_amount {
            return Err(ErrorCode::InvalidDepositAmount.into());
        }

        let transfer_instruction = Transfer{
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.escrow_vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        //start main staking process - deposit token
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, transfer_instruction);

        token::transfer(cpi_ctx, deposit_amount)?;

        // update package status
        staking_storage.packages[package_index as usize].total_locked_amount = package.total_locked_amount + deposit_amount;

        // update stake log
        let clock = Clock::get();
        let timestamp = clock.unwrap().unix_timestamp;

        let stake_log = StakeLog {
            id: staking_storage.stake_logs.len() as u8,
            staker: ctx.accounts.from.to_account_info().key(),
            package_index: package_index,
            stake_amount: deposit_amount,
            stake_timestamp: timestamp,
            terminated: false
        };

        staking_storage.stake_logs.push(stake_log);

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, escrow_bump: u8, stake_id: u8) -> Result<()> {
        // validate package index
        let packages = & ctx.accounts.staking_storage.packages;

        // check if user is valid staker and time lock
        let stake_logs = & ctx.accounts.staking_storage.stake_logs;

        if stake_id >= stake_logs.len() as u8 {
            return Err(ErrorCode::NonExistStake.into());
        }

        let stake_log = &stake_logs[stake_id as usize];

        if stake_log.staker == ctx.accounts.to.to_account_info().key() {
            // check if active stake
            if stake_log.terminated == true {
                return Err(ErrorCode::StakeAlreadyTerminated.into());
            }

            // check time lock
            let clock = Clock::get();
            let timestamp = clock.unwrap().unix_timestamp;
            let expected_timestamp = stake_log.stake_timestamp + packages[stake_log.package_index as usize].period;
        
            if expected_timestamp > timestamp {
                return Err(ErrorCode::InvalidLockTime.into());
            }
        }
        else {
            return Err(ErrorCode::AccountNeverStaked.into());
        }

        // create/configure withdraw transaction
        let mint_key = &mut ctx.accounts.mint.key();
        let seeds = &["escrow_vault".as_bytes(), mint_key.as_ref(), &[escrow_bump]];
        let signer_seeds = &[&seeds[..]];

        let transfer_instruction = Transfer{
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.to.to_account_info(),
            authority: ctx.accounts.escrow_vault.to_account_info(),
        };

        // start main withdraw/reward process - deposit token
        let reward_amount = stake_log.stake_amount / 10000 * packages[stake_log.package_index as usize].percentage;
        let withdraw_amount = stake_log.stake_amount + reward_amount;

        let cpi_program = ctx.accounts.token_program.to_account_info();

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, transfer_instruction, signer_seeds);

        token::transfer(cpi_ctx, withdraw_amount)?;

        // update stake log
        let staking_storage = &mut ctx.accounts.staking_storage;
        staking_storage.stake_logs[stake_id as usize].terminated = true;

        Ok(())
    }

    pub fn charge_escrow(ctx: Context<EscrowCharge>, deposit_amount: u64) -> Result<()> {
        let transfer_instruction = Transfer{
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.escrow_vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();

        let cpi_ctx = CpiContext::new(cpi_program, transfer_instruction);

        token::transfer(cpi_ctx, deposit_amount)?;
        Ok(())
    }

    pub fn release_escrow(ctx: Context<EscrowRelease>, escrow_bump: u8, release_amount: u64) -> Result<()> {
        let staking_storage = &mut ctx.accounts.staking_storage;

        if *ctx.accounts.signer.key == staking_storage.maintainer {
            let mint_key = &mut ctx.accounts.mint.key();
            let seeds = &["escrow_vault".as_bytes(), mint_key.as_ref(), &[escrow_bump]];
            let signer_seeds = &[&seeds[..]];

            let transfer_instruction = Transfer{
                from: ctx.accounts.escrow_vault.to_account_info(),
                to: ctx.accounts.to.to_account_info(),
                authority: ctx.accounts.escrow_vault.to_account_info(),
            };

            let cpi_program = ctx.accounts.token_program.to_account_info();

            let cpi_ctx = CpiContext::new_with_signer(cpi_program, transfer_instruction, signer_seeds);

            token::transfer(cpi_ctx, release_amount)?;
            Ok(())
        }
        else {
            return Err(ErrorCode::NeedMaintainerRole.into());
        }
    }

    pub fn change_percentage(ctx: Context<ChangePercentage>, package_index: u8, percentage: u64) -> Result<()> {
        let staking_storage = &mut ctx.accounts.staking_storage;
        if *ctx.accounts.signer.key == staking_storage.maintainer {
    
            staking_storage.packages[package_index as usize].percentage = percentage;
    
            Ok(())
        }
        else {
            return Err(ErrorCode::NeedMaintainerRole.into());
        }

    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init,
        payer = signer,
        space=size_of::<StakingStorage>() + 8000,
        seeds = [],
        bump)]
    pub staking_storage: Account<'info, StakingStorage>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,
    
    #[account(
        init,
        payer = signer,
        owner = token_program.key(),
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        // rent_exempt = enforce,
        token::mint = mint,
        token::authority = escrow_vault,
        bump)]
    pub escrow_vault: Account<'info, TokenAccount>,

    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub from: UncheckedAccount<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    // #[account(mut)]
    // pub to: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [], bump)]
    pub staking_storage: Account<'info, StakingStorage>,

    pub system_program: Program<'info, System>,

    #[account(mut,
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        bump)]
    pub escrow_vault: Account<'info, TokenAccount>,
        
    /// Token mint.
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct EscrowCharge<'info> {
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub from: UncheckedAccount<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    // #[account(mut)]
    // pub to: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(mut,
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        bump)]
    pub escrow_vault: Account<'info, TokenAccount>,
        
    /// Token mint.
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct EscrowRelease<'info> {
    #[account(address = token::ID)]
    pub token_program: Program<'info, Token>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub to: UncheckedAccount<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    // #[account(mut)]
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(mut, seeds = [], bump)]
    pub staking_storage: Account<'info, StakingStorage>,

    #[account(mut,
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        bump)]
    pub escrow_vault: Account<'info, TokenAccount>,
        
    /// Token mint.
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct ChangePercentage<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, seeds = [], bump)]
    pub staking_storage: Account<'info, StakingStorage>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub token_program: Program<'info, Token>,

    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub to: UncheckedAccount<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [], bump)]
    pub staking_storage: Account<'info, StakingStorage>,

    #[account(mut,
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        bump)]
    pub escrow_vault: Account<'info, TokenAccount>,

    /// Token mint.
    pub mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Package {
    pub name: String,
    pub max_deposit_amount: u64,
    pub total_locked_amount: u64,
    pub period: i64,
    pub apr: u64,
    pub percentage: u64
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct StakeLog {
    pub id: u8,
    pub staker: Pubkey,
    pub package_index: u8,
    pub stake_amount: u64,
    // #[account(address = solana_program::sysvar::clock::ID)]
    pub stake_timestamp: i64,
    pub terminated: bool
}

#[account]
pub struct StakingStorage {
    packages: Vec<Package>,
    stake_logs: Vec<StakeLog>,
    maintainer: Pubkey
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid package index. It must be 0 ~ 2")]
    InvalidPackageIndex,
    #[msg("Stake does not exist")]
    NonExistStake,
    #[msg("Invalid deposit amount. Deposit amount over the maximum allowed")]
    InvalidDepositAmount,
    #[msg("Account never staked")]
    AccountNeverStaked,
    #[msg("Lock time period is not satisfied")]
    InvalidLockTime,
    #[msg("Stake already terminated")]
    StakeAlreadyTerminated,
    #[msg("Need Maintainer Role for this action")]
    NeedMaintainerRole
}