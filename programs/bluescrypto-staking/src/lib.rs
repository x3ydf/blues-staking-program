use solana_program::*;
use anchor_lang::prelude::*;
use anchor_lang::prelude::{ declare_id, borsh };
use solana_program::clock::Clock;
use std::mem::size_of;
use anchor_spl::token::{self, Mint, TokenAccount, Token, Transfer};

declare_id!("2Q16CbW78EsUpmAkizJJkMVaeCG9QtvD9CjBwVBZcoCv");

#[program]
pub mod bluescrypto_staking {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let package_a = Package {
            name: String::from("BLUES Pebble Pounch"),
            max_deposit_amount: 100000000,
            apr: 20,
            period: 60 * 60 * 24 * 30,
            slot_limit: 80,
            slot_count: 0
        };

        let package_b = Package {
            name: String::from("Blue Wheel Guild"),
            max_deposit_amount: 75000000,
            apr: 30,
            period: 60 * 60 * 24 * 30 * 2,
            slot_limit: 70,
            slot_count: 0
        };

        let package_c = Package {
            name: String::from("Burrower's Bounty"),
            max_deposit_amount: 50000000,
            apr: 45,
            period: 60 * 60 * 24 * 30 * 3,
            slot_limit: 60,
            slot_count: 0
        };

        let staking_storage = &mut ctx.accounts.staking_storage;
        staking_storage.packages.push(package_a);
        staking_storage.packages.push(package_b);
        staking_storage.packages.push(package_c);

        Ok(())
    }

    pub fn stake(ctx: Context<Deposit>, package_index: u8, deposit_amount: u64) -> Result<()> {
        let transfer_instruction = Transfer{
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.escrow_vault.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };
        
        let staking_storage = &mut ctx.accounts.staking_storage;
        let packages = & staking_storage.packages;
        let package = & packages[package_index as usize];
        
        // validate package index
        if package_index >= packages.len() as u8{
            return Err(ErrorCode::InvalidPackageIndex.into());
        }

        // check if user already have stake on same package
        for stake_log in  staking_storage.stake_logs.iter() {
            if stake_log.staker == ctx.accounts.from.to_account_info().key() && package_index == stake_log.package_index && stake_log.terminated == false {
                return Err(ErrorCode::AccountAlreadyStaked.into());
            }
            else {
                continue;
            }
        }

        // check if package slot fulfilled
        if package.slot_count == package.slot_limit {
            return Err(ErrorCode::PackageSlotFulFilled.into());
        }

        // check if deposit amount is valid
        if deposit_amount > package.max_deposit_amount {
            return Err(ErrorCode::InvalidDepositAmount.into());
        }

        //start main staking process - deposit token
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, transfer_instruction);

        token::transfer(cpi_ctx, deposit_amount)?;

        // update stake log
        let clock = Clock::get();
        let timestamp = clock.unwrap().unix_timestamp;

        let stake_log = StakeLog {
            staker: ctx.accounts.from.to_account_info().key(),
            package_index: package_index,
            stake_timestamp: timestamp,
            terminated: false
        };

        staking_storage.stake_logs.push(stake_log);

        // update package state
        let slot_count = staking_storage.packages[package_index as usize].slot_count;
        staking_storage.packages[package_index as usize].slot_count = slot_count + 1;

        Ok(())
    }

    pub fn withdraw(ctx: Context<Withdraw>, escrow_bump: u8, package_index: u8) -> Result<()> {
        // validate package index
        let packages = & ctx.accounts.staking_storage.packages;

        if package_index >= packages.len() as u8{
            return Err(ErrorCode::InvalidPackageIndex.into());
        }

        // check if user is valid staker and time lock
        let stake_logs = & ctx.accounts.staking_storage.stake_logs;
        let mut is_valid_staker = false;
        let mut active_stake_log: &StakeLog;
        let mut log_index: usize = 0;
        for stake_log in  stake_logs.iter() {
            if stake_log.staker == ctx.accounts.to.to_account_info().key() && package_index == stake_log.package_index {
                is_valid_staker = true;
                active_stake_log = stake_log;

                log_index = stake_logs.iter().position(|x| x.package_index == stake_log.package_index && x.staker == stake_log.staker).unwrap_or(0) as usize;

                // check if active stake
                if stake_log.terminated == true {
                    return Err(ErrorCode::StakeAlreadyTerminated.into());
                }

                // check time lock
                let clock = Clock::get();
                let timestamp = clock.unwrap().unix_timestamp;
                let expected_timestamp = active_stake_log.stake_timestamp + packages[package_index as usize].period;
            
                if expected_timestamp > timestamp {
                    return Err(ErrorCode::InvalidLockTime.into());
                }
            }
            else {
                continue;
            }
        }
        if is_valid_staker == false {
            return Err(ErrorCode::AccountNeverStaked.into());
        }

        let mint_key = &mut ctx.accounts.mint.key();
        let seeds = &["escrow_vault".as_bytes(), mint_key.as_ref(), &[escrow_bump]];
        let signer_seeds = &[&seeds[..]];

        let transfer_instruction = Transfer{
            from: ctx.accounts.escrow_vault.to_account_info(),
            to: ctx.accounts.to.to_account_info(),
            authority: ctx.accounts.escrow_vault.to_account_info(),
        };

        // start main withdraw/reward process - deposit token
        let withdraw_amount = ctx.accounts.staking_storage.packages[package_index as usize].apr;

        let cpi_program = ctx.accounts.token_program.to_account_info();

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, transfer_instruction, signer_seeds);

        token::transfer(cpi_ctx, withdraw_amount)?;

        // update stake log
        let staking_storage = &mut ctx.accounts.staking_storage;
        staking_storage.stake_logs[log_index].terminated = true;

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

    #[account(init,
        payer = signer,
        owner = token_program.key(),
        seeds = [b"escrow_vault".as_ref(), mint.key().as_ref()],
        rent_exempt = enforce,
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

    // #[account(mut)]
    // pub signer: Signer<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Package {
    pub name: String,
    pub max_deposit_amount: u64,
    pub period: i64,
    pub apr: u64,
    pub slot_limit: u8,
    pub slot_count: u8
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UltimatePackage {
    pub name: String,
    pub apy: u64,
    pub period: i64
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct StakeLog {
    pub staker: Pubkey,
    pub package_index: u8,

    // #[account(address = solana_program::sysvar::clock::ID)]
    pub stake_timestamp: i64,
    pub terminated: bool
}

#[account]
pub struct StakingStorage {
    packages: Vec<Package>,
    stake_logs: Vec<StakeLog>,
    ultimate_package: UltimatePackage
}

#[derive(Accounts)]
pub struct SetStakingStorage<'info> {
    #[account(mut, seeds = [], bump)]
    pub staking_storage: Account<'info, StakingStorage>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid package index. It must be 0 ~ 5")]
    InvalidPackageIndex,
    #[msg("Invalid deposit amount. Deposit amount over the maximum allowed")]
    InvalidDepositAmount,
    #[msg("Account already staked on same package")]
    AccountAlreadyStaked,
    #[msg("Package slot fulfilled")]
    PackageSlotFulFilled,
    #[msg("Account never staked")]
    AccountNeverStaked,
    #[msg("Lock time period is not satisfied")]
    InvalidLockTime,
    #[msg("Stake already terminated")]
    StakeAlreadyTerminated,
    #[msg("Limited staking is still available")]
    UltimateStakingNotAvailable
}