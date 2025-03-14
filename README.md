# sst

# Speed Staking Token ($SST)

## 🚀 Overview

The **Speed Staking Token ($SST)** is a **staking-based execution priority system** built on **Solana**, optimized for **high-frequency traders (HFT), institutions, and market makers**. By staking $SST tokens, traders gain **priority execution**, **reduced fees**, and **bonus rewards** for ultra-fast transactions. The protocol also integrates **governance mechanisms, dual staking pools, and liquidity incentives** to enhance its utility and decentralization.

---

## 📌 Features

### ✅ Staking Mechanism
- Users **stake $SST** to unlock **priority execution** in Solana AMM pools.
- HFT traders with **higher staked amounts** receive **lower fees** and **higher order priority**.
- Supports **dual staking with USDC** for diversified yield opportunities.

### ✅ Dynamic Fee Discount Calculation
- The more tokens a user stakes, the greater their **fee discount**.
- Locked staking grants **additional fee reductions**.
- **VIP Multiplier:** Traders holding over **100,000 SST** receive **exclusive VIP discounts**.

### ✅ Auto-Compounding Rewards
- Staked rewards are **automatically reinvested** into the user's staking balance, increasing yield over time.
- Users can toggle the **auto-restake** feature.

### ✅ Time-Locked Staking Tiers
- Users can **lock tokens for 30, 90, or 180 days** to **boost rewards** and **increase execution priority**.
- **Prevents flash loan abuse** and **ensures long-term participation**.

### ✅ High-Frequency Trading (HFT) Execution Priority
- Orders executed **within 100ms** earn **additional $SST incentives**.
- **Ultra-fast trades (<50ms)** receive **bonus fee discounts**.
- Designed for **market makers, institutions, and algorithmic traders**.

### ✅ Liquidity Provider (LP) Yield Boost
- Liquidity providers (LPs) **earn $SST rewards** for supplying capital to **fast-execution pools**.
- **Progressive APY scaling** based on LP contributions.

### ✅ Governance & Fee Distribution
- Users can create **governance proposals** to adjust **protocol fees, incentives, and execution logic**.
- Staking grants **voting power**, with **locked stakes increasing influence**.

### ✅ Borrowing Against Staked SST
- Users can **borrow up to 50%** of their staked SST amount.
- Borrowed tokens **must be repaid** to prevent liquidation.

### ✅ Flash Loan & Sybil Attack Prevention
- Locking periods prevent **Sybil attacks** and ensure **fair staking**.
- **Progressive penalty structure** for early unstaking.

### ✅ Dual Staking & Yield Farming
- **Dual staking support** (SST + USDC) increases **protocol liquidity**.
- Users can **deposit LP tokens** to earn **staking rewards**.

### ✅ Insurance Fund for Protocol Security
- Users can **donate SST tokens** to a governance-backed **insurance fund**.
- Ensures **protocol sustainability** and **security**.

---

## ⚙️ Smart Contract (Program) Architecture

### 📄 **Staking Contract (`lib.rs`)**
The **Solana smart contract** is built using the **Anchor framework**.

### **🔹 Instructions**
#### 1️⃣ `stake(amount: u64)`
- Stakes $SST tokens with **no lock period**.
- Provides **basic fee discounts** but **no bonus rewards**.

#### 2️⃣ `stake_with_lock(amount: u64, lock_period: u64)`
- Stakes $SST tokens with a **lock period** (30, 90, or 180 days).
- Grants **additional fee discounts** and **execution priority**.

#### 3️⃣ `unstake(amount: u64)`
- Withdraws **staked $SST** (if unlocked).
- **Progressive vesting** applies to locked stakes.
- Early unstake penalty applies if unstaking before **7 days**.

#### 4️⃣ `execute_trade(order_execution_time: u64)`
- Applies **dynamic fee discounts** based on staking level.
- **Rewards ultra-fast execution** (trades within **100ms**).
- **VIP traders** get additional incentives.

#### 5️⃣ `claim_rewards(liquidity_provided: u64)`
- Claims and **auto-compounds** staking rewards.
- **Boosts LP yield** based on liquidity contribution.

#### 6️⃣ `borrow(amount: u64)`
- Allows borrowing **up to 50%** of staked SST.
- Requires repayment to avoid liquidation.

#### 7️⃣ `toggle_auto_restake(enabled: bool)`
- Enables or disables **auto-compounding** of rewards.

#### 8️⃣ `stake_dual(sst_amount: u64, usdc_amount: u64)`
- Allows users to **stake both SST and USDC**.
- **Dual staking** enhances liquidity and earns additional rewards.

#### 9️⃣ `deposit_lp(lp_amount: u64)`
- Users can **deposit LP tokens** to earn **staking rewards**.

#### 🔟 `flash_loan(amount: u64)`
- Allows traders to **borrow liquidity instantly** for **high-frequency trading**.

#### 11️⃣ `slash_stake(slash_percentage: u64)`
- Governance-only function to **slash stake** for **bad actors or Sybil attackers**.

#### 12️⃣ `donate_insurance(amount: u64)`
- Users can **donate SST** to the **insurance fund** for **protocol security**.

#### 13️⃣ `create_proposal(description: String)`
- Allows users to **submit governance proposals** for protocol upgrades.

#### 14️⃣ `vote_proposal(support: bool)`
- Stakeholders **vote on governance proposals** using their staking balance.

---

## 📜 Security & Risk Management
- **Reentrancy protection** is enabled for all staking-related transactions.
- **Governance voting** prevents **arbitrary fee changes**.
- **VIP and institutional safeguards** ensure **fair execution priority**.
- **Flash loan risk mitigation** via **minimum stake duration rules**.

---

# LiCENSE: MIT LICENSE

---
