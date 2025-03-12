use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("FGbGLGj7h1sTpfescPQvDteMj8mQpe9HNWd7V1xvyMnM"); //program id from solana playground

#[program]
pub mod sst {
    use super::*;

    /// Standard staking instruction (no lock period)
    /// This version allows immediate unstaking but offers no enhanced fee discount.
    pub fn stake(ctx: Context<StakeAccounts>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;

        // Transfer tokens from the staker's token account to the vault.
        let cpi_accounts = Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        // Initialize or update staking record.
        stake_info.staker = ctx.accounts.staker.key();
        stake_info.amount = stake_info
            .amount
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        stake_info.last_staked_time = clock.unix_timestamp;
        // Non-locked stake: no lock period.
        stake_info.lock_period = 0;
        stake_info.locked_until = clock.unix_timestamp;
        Ok(())
    }

    /// Staking instruction with a lock period (30, 90, or 180 days)
    /// This prevents flash loan abuse and grants enhanced benefits.
    pub fn stake_with_lock(ctx: Context<StakeAccounts>, amount: u64, lock_period: u64) -> Result<()> {
        // Allowed lock periods in seconds: 30, 90, or 180 days.
        let allowed_periods: Vec<u64> = vec![
            30 * 24 * 60 * 60,
            90 * 24 * 60 * 60,
            180 * 24 * 60 * 60,
        ];
        require!(allowed_periods.contains(&lock_period), ErrorCode::InvalidLockPeriod);
        
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        
        // Transfer tokens from the staker to the vault.
        let cpi_accounts = Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(CpiContext::new(cpi_program, cpi_accounts), amount)?;

        // Update staking record with lock details.
        stake_info.staker = ctx.accounts.staker.key();
        stake_info.amount = stake_info
            .amount
            .checked_add(amount)
            .ok_or(ErrorCode::Overflow)?;
        stake_info.last_staked_time = clock.unix_timestamp;
        stake_info.lock_period = lock_period;
        stake_info.locked_until = clock.unix_timestamp
            .checked_add(lock_period as i64)
            .ok_or(ErrorCode::Overflow)?;
        Ok(())
    }

    /// Unstake instruction: allows withdrawal of staked tokens.
    /// For locked stakes, ensures that the lock period has expired.
    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        let stake_info = &mut ctx.accounts.stake_info;
        let clock = Clock::get()?;
        require!(stake_info.amount >= amount, ErrorCode::InsufficientStakedAmount);
        // If staked with a lock, enforce the locked duration.
        if stake_info.lock_period > 0 {
            require!(clock.unix_timestamp >= stake_info.locked_until, ErrorCode::TokensLocked);
        }

        // Transfer tokens back from the vault to the staker.
        let seeds = &[b"vault".as_ref()];
        let signer = &[&seeds[..]];
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.staker_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        token::transfer(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, signer),
            amount,
        )?;

        stake_info.amount = stake_info
            .amount
            .checked_sub(amount)
            .ok_or(ErrorCode::Underflow)?;
        Ok(())
    }

    /// Execute trade instruction: applies dynamic fee discounts based on staking.
    /// Also provides bonus incentives for ultra-fast execution (<= 100ms).
    pub fn execute_trade(ctx: Context<ExecuteTrade>, order_execution_time: u64) -> Result<()> {
        let stake_info = &ctx.accounts.stake_info;
        let clock = Clock::get()?;
        let staking_duration = clock.unix_timestamp
            .checked_sub(stake_info.last_staked_time)
            .unwrap_or(0);
        let fee_discount = if stake_info.lock_period > 0 {
            // Only locked stakes receive enhanced fee discounts.
            calculate_fee_discount(stake_info.amount, staking_duration)
        } else {
            0
        };

        msg!("Calculated fee discount: {}%", fee_discount);

        if order_execution_time <= 100 {
            msg!("Trade executed within 100ms: bonus incentives applied.");
        } else {
            msg!("Trade executed without bonus incentive.");
        }
        Ok(())
    }

    /// Claim rewards instruction: auto-compounds rewards into the staked amount.
    /// Incorporates an LP yield boost if liquidity is provided.
    pub fn claim_rewards(ctx: Context<ClaimRewards>, liquidity_provided: u64) -> Result<()> {
        // Placeholder reward calculation: base_reward can be determined by more complex logic.
        let base_reward: u64 = 100;
        let lp_boost = lp_reward_boost(liquidity_provided);
        let total_reward = base_reward
            .checked_add(lp_boost)
            .ok_or(ErrorCode::Overflow)?;
        
        // Auto-compound: add rewards directly to the staked balance.
        let stake_info = &mut ctx.accounts.stake_info;
        stake_info.amount = stake_info
            .amount
            .checked_add(total_reward)
            .ok_or(ErrorCode::Overflow)?;
        msg!("Rewards auto-compounded: {} tokens added", total_reward);
        Ok(())
    }

    /// Governance instruction: creates a proposal for fee distribution or protocol changes.
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
}

/// Helper function to calculate a dynamic fee discount based on staked amount and duration.
/// Example: 1000 SST = 1% base discount, plus 1% bonus per 30 days, capped at 50%.
fn calculate_fee_discount(staked_amount: u64, staking_duration: i64) -> u64 {
    let base_discount = staked_amount / 1000;
    let duration_bonus = (staking_duration / (30 * 24 * 60 * 60)) as u64;
    std::cmp::min(base_discount + duration_bonus, 50)
}

/// Helper function for LP yield boost: every 10,000 tokens of liquidity provided grants bonus tokens.
fn lp_reward_boost(liquidity_provided: u64) -> u64 {
    let boost = liquidity_provided / 10_000;
    std::cmp::min(boost, 20)
}

#[derive(Accounts)]
pub struct StakeAccounts<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    // PDA to track staking details; created if not already present.
    #[account(
        init,
        payer = staker,
        space = 8 + StakeInfo::LEN,
        seeds = [b"stake", staker.key().as_ref()],
        bump
    )]
    pub stake_info: Account<'info, StakeInfo>,

    // Token account holding the staker's $SST tokens.
    #[account(mut)]
    pub staker_token_account: Box<Account<'info, TokenAccount>>,

    // Vault token account where staked tokens are held.
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

    // Existing staking record.
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

    // Staking record used to determine trade priority and fee discount.
    #[account(seeds = [b"stake", staker.key().as_ref()], bump)]
    pub stake_info: Account<'info, StakeInfo>,
    // Additional AMM pool accounts may be included as needed.
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub staker: Signer<'info>,

    // Staking record to which rewards will be compounded.
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

#[account]
pub struct StakeInfo {
    pub staker: Pubkey,
    pub amount: u64,
    pub last_staked_time: i64,
    pub lock_period: u64,  // in seconds; 0 indicates non-locked staking
    pub locked_until: i64, // timestamp when staked tokens can be withdrawn
}

impl StakeInfo {
    // Total space: 32 (Pubkey) + 8 (amount) + 8 (last_staked_time) + 8 (lock_period) + 8 (locked_until)
    const LEN: usize = 32 + 8 + 8 + 8 + 8;
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
    // Estimated space: Pubkey (32) + description (4 + up to 200 bytes) + votes_for (8) + votes_against (8) + created_at (8)
    const LEN: usize = 32 + 4 + 200 + 8 + 8 + 8;
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
}
