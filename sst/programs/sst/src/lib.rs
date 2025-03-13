use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("FGbGLGj7h1sTpfescPQvDteMj8mQpe9HNWd7V1xvyMnM");

/// Minimum duration (in seconds) a non-locked stake must remain before unstaking without penalty (7 days)
const MIN_NON_LOCKED_STAKE_DURATION: i64 = 7 * 24 * 60 * 60;
/// VIP threshold: 100,000 SST (assuming 6 decimals)
const VIP_THRESHOLD: u64 = 100_000 * 1_000_000;

#[program]
pub mod sst {
    use super::*;

    /// Standard staking instruction (no lock period).
    pub fn stake(ctx: Context<StakeAccounts>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        require!(!stake_info.locked, ErrorCode::ReentrancyDetected);
        stake_info.locked = true;

        let cpi_accounts = Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        stake_info.staker = ctx.accounts.staker.key();
        stake_info.amount = stake_info.amount.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        stake_info.last_staked_time = clock.unix_timestamp;
        stake_info.lock_period = 0;
        stake_info.locked_until = clock.unix_timestamp;
        stake_info.borrowed_amount = 0;
        stake_info.locked = false;
        stake_info.auto_restake = false;
        Ok(())
    }

    /// Staking instruction with a lock period (30, 90, or 180 days).
    pub fn stake_with_lock(ctx: Context<StakeAccounts>, amount: u64, lock_period: u64) -> Result<()> {
        let allowed_periods: Vec<u64> = vec![
            30 * 24 * 60 * 60,
            90 * 24 * 60 * 60,
            180 * 24 * 60 * 60,
        ];
        require!(allowed_periods.contains(&lock_period), ErrorCode::InvalidLockPeriod);
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        require!(!stake_info.locked, ErrorCode::ReentrancyDetected);
        stake_info.locked = true;

        let cpi_accounts = Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        stake_info.staker = ctx.accounts.staker.key();
        stake_info.amount = stake_info.amount.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        stake_info.last_staked_time = clock.unix_timestamp;
        stake_info.lock_period = lock_period;
        stake_info.locked_until = clock.unix_timestamp.checked_add(lock_period as i64).ok_or(ErrorCode::Overflow)?;
        stake_info.borrowed_amount = 0;
        stake_info.locked = false;
        stake_info.auto_restake = false;
        Ok(())
    }

    /// Unstake instruction with progressive (linear vesting) unlocking.
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        require!(stake_info.amount >= amount, ErrorCode::InsufficientStakedAmount);

        if stake_info.lock_period > 0 {
            let time_elapsed = clock.unix_timestamp
                .checked_sub(stake_info.last_staked_time)
                .ok_or(ErrorCode::Underflow)?;
            let unlock_ratio = if time_elapsed >= stake_info.lock_period as i64 { 
                1.0 
            } else {
                time_elapsed as f64 / stake_info.lock_period as f64
            };
            let unlocked_amount = (stake_info.amount as f64 * unlock_ratio).floor() as u64;
            require!(amount <= unlocked_amount, ErrorCode::TokensLocked);
            let seeds = &[b"vault".as_ref()];
            let signer = &[&seeds[..]];
            let cpi_accounts = Transfer {
                from: ctx.accounts.vault_token_account.to_account_info(),
                to: ctx.accounts.staker_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), amount)?;
        } else {
            if clock.unix_timestamp - stake_info.last_staked_time < MIN_NON_LOCKED_STAKE_DURATION {
                let penalty = amount.checked_mul(2).ok_or(ErrorCode::Overflow)?
                    .checked_div(100).ok_or(ErrorCode::Underflow)?;
                msg!("Early unstake penalty applied: {} tokens withheld", penalty);
                let amount_to_transfer = amount.checked_sub(penalty).ok_or(ErrorCode::Underflow)?;
                let seeds = &[b"vault".as_ref()];
                let signer = &[&seeds[..]];
                let cpi_accounts = Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), amount_to_transfer)?;
            } else {
                let seeds = &[b"vault".as_ref()];
                let signer = &[&seeds[..]];
                let cpi_accounts = Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.staker_token_account.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                };
                let cpi_program = ctx.accounts.token_program.to_account_info();
                token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), amount)?;
            }
        }
        stake_info.amount = stake_info.amount.checked_sub(amount).ok_or(ErrorCode::Underflow)?;
        Ok(())
    }

    /// Execute trade instruction: applies dynamic fee discounts based on staking, VIP boost,
    /// duration bonus, and extra bonus for ultra-fast execution.
    pub fn execute_trade(ctx: Context<ExecuteTrade>, order_execution_time: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        let staking_duration = clock.unix_timestamp
            .checked_sub(stake_info.last_staked_time)
            .unwrap_or(0);
        let fee_discount = if stake_info.lock_period > 0 {
            calculate_fee_discount(stake_info.amount, staking_duration)
        } else {
            0
        };
        let vip_mult = vip_multiplier(stake_info.amount);
        let mut adjusted_fee_discount = fee_discount * vip_mult / 100;
        msg!("Base fee discount: {}%, VIP multiplier: {}%", fee_discount, vip_mult);

        let duration_priority_bonus = if staking_duration >= 180 * 24 * 60 * 60 {
            5
        } else if staking_duration >= 90 * 24 * 60 * 60 {
            3
        } else if staking_duration >= 30 * 24 * 60 * 60 {
            1
        } else {
            0
        };
        adjusted_fee_discount = adjusted_fee_discount.checked_add(duration_priority_bonus).ok_or(ErrorCode::Overflow)?;
        msg!("Duration priority bonus: {}%", duration_priority_bonus);

        if stake_info.amount >= VIP_THRESHOLD {
            adjusted_fee_discount = adjusted_fee_discount.checked_add(10).ok_or(ErrorCode::Overflow)?;
            msg!("Institutional VIP boost applied.");
        }

        if order_execution_time <= 50 {
            msg!("Ultra-fast execution (<= 50ms) achieved: extra bonus applied.");
            adjusted_fee_discount = adjusted_fee_discount.checked_add(5).ok_or(ErrorCode::Overflow)?;
            stake_info.amount = stake_info.amount.checked_add(20).ok_or(ErrorCode::Overflow)?;
        } else if order_execution_time <= 100 {
            msg!("Trade executed within 100ms: bonus incentives applied.");
        } else {
            msg!("Trade executed without bonus incentive.");
        }
        msg!("Adjusted fee discount: {}%", adjusted_fee_discount);
        Ok(())
    }

    /// Claim rewards instruction with auto-compounding and progressive APY scaling.
    pub fn claim_rewards(ctx: Context<ClaimRewards>, liquidity_provided: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        let staking_duration = clock.unix_timestamp
            .checked_sub(stake_info.last_staked_time)
            .unwrap_or(0);
        let months = staking_duration / (30 * 24 * 60 * 60);
        let progressive_bonus = months * 10;
        let base_reward: i64 = 100 + progressive_bonus;
        let lp_boost: u64 = lp_reward_boost(liquidity_provided);
        let total_reward: i64 = base_reward.checked_add(lp_boost.try_into().unwrap()).ok_or(ErrorCode::Overflow)?;
        if stake_info.auto_restake {
            stake_info.amount = stake_info.amount.checked_add(total_reward.try_into().unwrap()).ok_or(ErrorCode::Overflow)?;
            msg!("Rewards auto-compounded: {} tokens added (Base: {}, LP Boost: {})", total_reward, base_reward, lp_boost);
        } else {
            let seeds = &[b"vault".as_ref()];
            let signer = &[&seeds[..]];
            let cpi_accounts = Transfer {
                from: ctx.accounts.reward_vault.to_account_info(),
                to: ctx.accounts.staker_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), total_reward.try_into().unwrap())?;
            msg!("Rewards claimed: {} tokens transferred", total_reward);
        }
        Ok(())
    }

    /// Governance instruction: creates a proposal for protocol changes.
    pub fn create_proposal(ctx: Context<CreateProposal>, description: String) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.description = description;
        proposal.votes_for = 0;
        proposal.votes_against = 0;
        proposal.created_at = Clock::get()?.unix_timestamp;
        msg!("New governance proposal created");
        Ok(())
    }

    /// Vote on a proposal.
    pub fn vote_proposal(ctx: Context<VoteProposal>, support: bool) -> Result<()> {
        let voting_power = calculate_voting_power(&ctx.accounts.stake_info);
        let proposal = &mut ctx.accounts.proposal;
        if support {
            proposal.votes_for = proposal.votes_for.checked_add(voting_power).ok_or(ErrorCode::Overflow)?;
        } else {
            proposal.votes_against = proposal.votes_against.checked_add(voting_power).ok_or(ErrorCode::Overflow)?;
        }
        msg!("Vote cast with power: {}", voting_power);
        Ok(())
    }

    /// Borrow instruction: allows borrowing up to 50% of staked SST.
    pub fn borrow(ctx: Context<Borrow>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let max_borrow = stake_info.amount.checked_div(2).ok_or(ErrorCode::Overflow)?;
        require!(amount <= max_borrow, ErrorCode::BorrowLimitExceeded);
        stake_info.borrowed_amount = stake_info.borrowed_amount.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        msg!("Borrowed {} tokens against stake", amount);
        Ok(())
    }

    /// Toggle the auto-restake option.
    pub fn toggle_auto_restake(ctx: Context<ToggleAutoRestake>, enabled: bool) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        stake_info.auto_restake = enabled;
        msg!("Auto-restake toggled to: {}", enabled);
        Ok(())
    }

    /// Dual staking pool: stake both SST and USDC.
    pub fn stake_dual(ctx: Context<StakeDual>, sst_amount: u64, usdc_amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        // Transfer SST.
        let cpi_accounts_sst = Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program.clone(), cpi_accounts_sst), sst_amount)?;
        // Transfer USDC.
        let cpi_accounts_usdc = Transfer {
            from: ctx.accounts.staker_usdc_token_account.to_account_info(),
            to: ctx.accounts.vault_usdc_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        token::transfer(CpiContext::new(cpi_program, cpi_accounts_usdc), usdc_amount)?;
        stake_info.staker = ctx.accounts.staker.key();
        stake_info.amount = stake_info.amount.checked_add(sst_amount).ok_or(ErrorCode::Overflow)?;
        stake_info.usdc_amount = stake_info.usdc_amount.checked_add(usdc_amount).ok_or(ErrorCode::Overflow)?;
        stake_info.last_staked_time = clock.unix_timestamp;
        stake_info.auto_restake = false;
        Ok(())
    }

    /// Deposit LP tokens for yield farming.
    pub fn deposit_lp(ctx: Context<DepositLP>, lp_amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let cpi_accounts = Transfer {
            from: ctx.accounts.staker_lp_token_account.to_account_info(),
            to: ctx.accounts.vault_lp_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), lp_amount)?;
        stake_info.lp_deposit = stake_info.lp_deposit.checked_add(lp_amount).ok_or(ErrorCode::Overflow)?;
        Ok(())
    }

    /// Flash loan: borrow tokens instantly against staked SST.
    pub fn flash_loan(ctx: Context<FlashLoan>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let max_flash = stake_info.amount.checked_div(2).ok_or(ErrorCode::Overflow)?;
        require!(amount <= max_flash, ErrorCode::BorrowLimitExceeded);
        let seeds = &[b"vault".as_ref()];
        let signer = &[&seeds[..]];
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.borrower_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new_with_signer(cpi_program, cpi_accounts, signer), amount)?;
        stake_info.borrowed_amount = stake_info.borrowed_amount.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        Ok(())
    }

    /// Slash stake as a penalty for Sybil attacks (governance only).
    pub fn slash_stake(ctx: Context<SlashStake>, slash_percentage: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let slash_amount = stake_info.amount.checked_mul(slash_percentage).ok_or(ErrorCode::Overflow)?
            .checked_div(100).ok_or(ErrorCode::Underflow)?;
        stake_info.amount = stake_info.amount.checked_sub(slash_amount).ok_or(ErrorCode::Underflow)?;
        msg!("Slashed {} tokens from stake", slash_amount);
        Ok(())
    }

    /// Donate to the governance-backed insurance fund.
    pub fn donate_insurance(ctx: Context<DonateInsurance>, amount: u64) -> Result<()> {
        let insurance_fund = &mut ctx.accounts.insurance_fund;
        let cpi_accounts = Transfer {
            from: ctx.accounts.donor_token_account.to_account_info(),
            to: ctx.accounts.insurance_fund_token_account.to_account_info(),
            authority: ctx.accounts.donor.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;
        insurance_fund.balance = insurance_fund.balance.checked_add(amount).ok_or(ErrorCode::Overflow)?;
        msg!("Donated {} tokens to the insurance fund", amount);
        Ok(())
    }
}

/// Helper: calculates dynamic fee discount.
fn calculate_fee_discount(staked_amount: u64, staking_duration: i64) -> u64 {
    let base_discount = staked_amount / 1000;
    let duration_bonus = (staking_duration / (30 * 24 * 60 * 60)) as u64;
    std::cmp::min(base_discount + duration_bonus, 50)
}

/// Helper: calculates LP yield boost.
fn lp_reward_boost(liquidity_provided: u64) -> u64 {
    let boost = liquidity_provided / 10_000;
    std::cmp::min(boost, 20)
}

/// Helper: returns a VIP multiplier based on staked amount.
fn vip_multiplier(staked_amount: u64) -> u64 {
    if staked_amount >= 10_000 * 1_000_000 {
        130
    } else if staked_amount >= 5_000 * 1_000_000 {
        115
    } else if staked_amount >= 1_000 * 1_000_000 {
        105
    } else {
        100
    }
}

/// Helper: calculates voting power based on staked amount and duration.
fn calculate_voting_power(stake_info: &StakeInfo) -> u64 {
    let clock = Clock::get().unwrap();
    let duration = clock.unix_timestamp.checked_sub(stake_info.last_staked_time).unwrap_or(0);
    let base_power = stake_info.amount;
    let bonus = base_power * ((duration / (30 * 24 * 60 * 60)) as u64) / 100;
    base_power.checked_add(bonus).unwrap_or(base_power)
}

#[derive(Accounts)]
pub struct StakeAccounts<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(
        init,
        payer = staker,
        space = 8 + StakeInfo::LEN,
        seeds = [b"stake", staker.key().as_ref()],
        bump
    )]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub staker_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub staker_token_account: Box<Account<'info, TokenAccount>>,
    
    #[account(mut)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExecuteTrade<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub staker_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub reward_vault: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ToggleAutoRestake<'info> {
    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,
    pub staker: Signer<'info>,
}

#[derive(Accounts)]
pub struct StakeDual<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(
        init,
        payer = staker,
        space = 8 + StakeInfo::LEN,
        seeds = [b"stake", staker.key().as_ref()],
        bump
    )]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub staker_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub staker_usdc_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub vault_usdc_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositLP<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub staker_lp_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub vault_lp_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct FlashLoan<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub vault_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub borrower_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: Derived PDA for the vault authority.
    pub vault_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SlashStake<'info> {
    #[account(mut)]
    pub gov_authority: Signer<'info>, // Governance authority

    /// The staker whose stake will be slashed.
    pub staker: AccountInfo<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,
}

#[derive(Accounts)]
pub struct VoteProposal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,

    #[account(mut)]
    pub stake_info: Account<'info, StakeInfo>,

    #[account(mut)]
    pub proposal: Account<'info, Proposal>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Borrow<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    #[account(mut, seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,

    #[account(
        init, 
        payer = proposer, 
        space = 8 + Proposal::LEN, 
        seeds = [b"proposal", proposer.key().as_ref(), proposer.to_account_info().key.as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DonateInsurance<'info> {
    #[account(mut)]
    pub donor: Signer<'info>,

    #[account(mut)]
    pub donor_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub insurance_fund_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut, seeds = [b"insurance_fund"], bump)]
    pub insurance_fund: Account<'info, InsuranceFund>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[account]
pub struct InsuranceFund {
    pub balance: u64,
}

impl InsuranceFund {
    const LEN: usize = 8;
}

#[account]
pub struct StakeInfo {
    pub staker: Pubkey,
    pub amount: u64,
    pub last_staked_time: i64,
    pub lock_period: u64,
    pub locked_until: i64,
    pub borrowed_amount: u64,
    pub locked: bool,
    pub auto_restake: bool,
    pub usdc_amount: u64,
    pub lp_deposit: u64,
}

impl StakeInfo {
    // Updated space: padded to 112 bytes.
    const LEN: usize = 112;
}

#[account]
pub struct Proposal {
    pub proposer: Pubkey,
    pub description: String,
    pub votes_for: u64,
    pub votes_against: u64,
    pub created_at: i64,
}

impl Proposal {
    const LEN: usize = 268;
}

#[error_code]
pub enum ErrorCode {
    #[msg("Arithmetic operation overflowed.")]
    Overflow,
    #[msg("Arithmetic operation underflowed.")]
    Underflow,
    #[msg("Insufficient staked amount to complete unstaking.")]
    InsufficientStakedAmount,
    #[msg("Tokens are still locked.")]
    TokensLocked,
    #[msg("Invalid lock period specified.")]
    InvalidLockPeriod,
    #[msg("Reentrancy detected.")]
    ReentrancyDetected,
    #[msg("Borrow limit exceeded.")]
    BorrowLimitExceeded,
}


