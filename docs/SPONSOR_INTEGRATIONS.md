# sponsors.md

## ETHMumbai 2026 — Sponsor Alignment Guide

This document explains the sponsors of ETHMumbai 2026, what each sponsor is looking for, and how our project architecture (GhostPay / CloakFund stealth payments) should integrate with them to maximize prize potential.

The purpose of this file is to help development agents understand:

• what each sponsor wants
• which integrations matter most
• which integrations should be prioritized
• how to structure the architecture so we can win multiple bounties

This file must be referenced during development decisions.


------------------------------------------------

## 1. Hackathon Core Themes

ETHMumbai focuses on three main themes.

Privacy  
DeFi  
AI

Our project primarily targets the **Privacy track**.

However, we can stack multiple sponsor prizes if we integrate their technology meaningfully.


------------------------------------------------

## 2. Full Sponsor List

Main sponsors relevant to our project:

BitGo  
ENS  
HeyElsa  
Fileverse  
Base


------------------------------------------------

# 3. BitGo (Highest Priority Sponsor)

Prize Pool:
$2,000 total

Best Privacy Application using BitGo  
Best DeFi Application using BitGo

This is the most important sponsor for our project.

Our architecture must demonstrate **deep integration with BitGo infrastructure**, not just basic API usage.


------------------------------------------------

## What BitGo Actually Wants

BitGo is an institutional crypto infrastructure company.

They provide:

• MPC wallet infrastructure
• transaction APIs
• custody solutions
• treasury management
• policy engines
• settlement systems

They want developers to build **enterprise-grade applications on top of BitGo wallets**.


------------------------------------------------

## What Judges Look For in BitGo Projects

Projects that win BitGo bounties usually demonstrate:

• real financial workflows
• treasury automation
• wallet infrastructure usage
• policy enforcement
• transaction automation

They do NOT want:

• basic wallet demos
• simple API calls
• superficial integrations


------------------------------------------------

## How Our Project Uses BitGo

GhostPay uses BitGo as the **custody and treasury layer**.

Flow:

Sender
→ stealth payment address
→ backend detects deposit
→ funds consolidated
→ BitGo treasury wallet


------------------------------------------------

## BitGo Integrations We Must Implement

The system should use the following BitGo features.

### 1. Wallet Infrastructure

BitGo wallet acts as the final treasury.

All incoming stealth payments are consolidated into a BitGo wallet.

This demonstrates enterprise custody integration.


------------------------------------------------

### 2. Transaction APIs

Sweeping funds from temporary wallets into the BitGo wallet must use BitGo transaction APIs.

This demonstrates automated transaction workflows.


------------------------------------------------

### 3. Webhook Notifications

BitGo webhooks can detect deposits.

Example:

Deposit detected
Address
Amount
Transaction hash

This can trigger automated processing.

Using webhooks shows proper integration with BitGo infrastructure.


------------------------------------------------

### 4. Policy Controls (Optional Advanced Feature)

BitGo allows transaction policies.

Example:

• require approval above certain amounts
• restrict withdrawals
• whitelist addresses

Demonstrating policy rules would strengthen the BitGo integration.


------------------------------------------------

## BitGo Integration Summary

Our project demonstrates:

privacy payments  
enterprise custody  
automated treasury flows

This aligns directly with the **Best Privacy Application using BitGo** bounty.


------------------------------------------------

# 4. ENS (Second Priority)

Prize Pool:
$2,000 total

Best Creative Use of ENS  
ENS integrations pool

ENS is the identity layer of Ethereum.

They want projects that use ENS in creative ways.


------------------------------------------------

## How Our Project Uses ENS

ENS will act as the **human-readable identity layer**.

Example:

alice.eth


------------------------------------------------

### ENS Use Cases in the Project

User identity

Instead of raw addresses, users interact with ENS names.

Payment links

Example:

ghostpay.com/pay/alice.eth

Stealth key discovery

ENS text records can store stealth meta-address data.

Example:

stealth_key record


------------------------------------------------

## Why ENS Integration Matters

This makes the system:

• easier to use
• human readable
• identity aware

ENS integration improves UX significantly.


------------------------------------------------

# 5. Base

Prize Pool:
$1,000

Best project deployed on Base.

Base is a Layer 2 network built by Coinbase.

They want applications deployed on Base.


------------------------------------------------

## Why Base Is Good For Our Project

Privacy payment systems generate many transactions.

Base offers:

• low gas fees
• fast confirmations
• scalable infrastructure


------------------------------------------------

## How We Use Base

Smart contracts deployed on Base.

Stealth payment addresses operate on Base.

Transaction sweeping occurs on Base.

All testnet demos should run on Base.


------------------------------------------------

# 6. Fileverse

Prize Pool:
$1,000

Fileverse promotes privacy-first alternatives to Big Tech collaboration tools.

Their technology focuses on:

encrypted storage  
decentralized documents  
secure collaboration


------------------------------------------------

## How Our Project Uses Fileverse

Fileverse will be used for **encrypted payment receipts**.

When a payment occurs:

receipt generated
→ encrypted
→ stored using Fileverse


------------------------------------------------

Example receipt:

transaction hash  
amount  
timestamp  
payment link id

The encrypted receipt proves payment occurred while keeping data private.


------------------------------------------------

# 7. HeyElsa (Optional)

Prize Pool:
$2,000

HeyElsa focuses on AI agents.

Our project does not require AI, but an optional integration could include:

AI monitoring of treasury flows

Example:

agent watches incoming payments  
detects anomalies  
alerts users

This would allow participation in the AI track.


------------------------------------------------

# 8. Sponsor Priority Order

For development focus, the priorities are:

1 BitGo  
2 ENS  
3 Base  
4 Fileverse  
5 HeyElsa (optional)


------------------------------------------------

# 9. Minimum Integration Requirements

To maximize prize potential we must demonstrate:

BitGo

wallet infrastructure  
transaction automation  
treasury custody

ENS

user identity  
payment links  
stealth key records

Base

deploy contracts on Base network  
execute demo transactions

Fileverse

store encrypted payment receipts


------------------------------------------------

# 10. Demo Narrative

During the demo we must clearly show sponsor integrations.

Example demo flow:

Bob creates a payment link.

ghostpay.com/pay/bob.eth

A stealth payment address is generated.

Alice sends ETH.

The blockchain shows only the stealth address.

Backend detects payment.

Funds automatically move into the BitGo treasury wallet.

An encrypted receipt is generated and stored via Fileverse.

Bob's ENS identity is displayed in the dashboard.

This shows meaningful integrations with:

BitGo  
ENS  
Base  
Fileverse


------------------------------------------------

# 11. Agent Instructions

When implementing the system, ensure:

BitGo APIs are used for wallet and transaction flows.

ENS names are integrated into user identity and payment links.

Contracts are deployed on Base.

Receipts are encrypted and stored using Fileverse.

Sponsor integrations must be functional and visible during the demo.


------------------------------------------------

End of sponsors.md
