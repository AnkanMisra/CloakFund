# Architecture

CloakFund consists of several major layers.

Frontend  
Backend (Rust)  
Data Layer (Convex)  
Blockchain  
Treasury  
Encrypted Storage

---

## System Overview

User Wallet  
↓  
Next.js Frontend  
↓  
Rust Backend API  
↓  
Stealth Address Generator & Convex Data Layer  
↓  
Base Blockchain  
↓  
BitGo MPC Treasury  
↓  
Fileverse Encrypted Storage

---

## Frontend

Built with Next.js and TypeScript.

Responsibilities:

wallet connection  
ENS identity input  
payment link generation  
dashboard balance view  
receipt decryption

---

## Backend & Data Layer

Implemented in Rust and Convex.

Responsibilities:

stealth address generation (Rust)  
blockchain deposit watcher (Rust)  
real-time data persistence and APIs (Convex)  
treasury consolidation (Rust)  
receipt encryption (Rust)  
external integrations
