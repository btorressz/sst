# sst

# Speed Staking Token ($SST)

## 🚀 Overview

The **Speed Staking Token ($SST)** is a **staking-based execution priority system** built on **Solana**, optimized for **high-frequency traders (HFT), institutions, and market makers**. By staking $SST tokens, traders gain **priority execution**, **reduced fees**, and **bonus rewards** for ultra-fast transactions.

## 📌 Features

### ✅ Staking Mechanism
- Users **stake $SST** to unlock **priority execution** in Solana AMM pools.
- HFT traders with **higher staked amounts** receive **lower fees** and **higher order priority**.

### ✅ Dynamic Fee Discount Calculation
- The more tokens a user stakes, the greater their **fee discount**.
- Locked staking grants **additional fee reductions**.

### ✅ Auto-Compounding Rewards
- Staked rewards are **automatically reinvested** into the user's staking balance, increasing yield over time.

### ✅ Time-Locked Staking Tiers
- Users can **lock tokens for 30, 90, or 180 days** to **boost rewards** and **increase execution priority**.
- **Prevents flash loan abuse** and **ensures long-term participation**.

### ✅ High-Frequency Trading (HFT) Execution Priority
- Orders executed **within 100ms** earn **additional $SST incentives**.
- Designed for **market makers, institutions, and algorithmic traders**.

### ✅ Liquidity Provider (LP) Yield Boost
- Liquidity providers (LPs) **earn $SST rewards** for supplying capital to **fast-execution pools**.

### ✅ Flash Loan Prevention
- Locking periods prevent **Sybil attacks** and ensure **fair staking**.

### ✅ Governance & Fee Distribution
- Users can create **governance proposals** to adjust **protocol fees, incentives, and execution logic**.

---

## ⚙️ Smart Contract(program) Architecture

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
- Locked tokens **cannot be unstaked early**.

#### 4️⃣ `execute_trade(order_execution_time: u64)`
- Applies **dynamic fee discounts** based on staking level.
- **Rewards ultra-fast execution** (trades within **100ms**).

#### 5️⃣ `claim_rewards(liquidity_provided: u64)`
- Claims and **auto-compounds** staking rewards.
- **Boosts LP yield** based on liquidity contribution.

#### 6️⃣ `create_proposal(description: String)`
- Allows users to **submit governance proposals** for protocol upgrades.

---

