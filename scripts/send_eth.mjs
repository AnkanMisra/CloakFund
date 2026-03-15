#!/usr/bin/env node
// scripts/send_eth.mjs
// Sends a small amount of ETH to a stealth address for testing.
// Usage: node scripts/send_eth.mjs <stealth_address>
//
// Requires SENDER_PRIVATE_KEY in .env (a Base Sepolia funded wallet)

import { ethers } from "ethers";
import { readFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const envPath = resolve(__dirname, "..", ".env");

// Parse .env manually (no dotenv dependency needed)
function loadEnv(path) {
    const env = {};
    try {
        const lines = readFileSync(path, "utf8").split("\n");
        for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed || trimmed.startsWith("#")) continue;
            const eqIdx = trimmed.indexOf("=");
            if (eqIdx === -1) continue;
            const key = trimmed.slice(0, eqIdx).trim();
            let val = trimmed.slice(eqIdx + 1).trim();
            // Strip inline comments
            const commentIdx = val.indexOf(" #");
            if (commentIdx !== -1) val = val.slice(0, commentIdx).trim();
            env[key] = val;
        }
    } catch { }
    return env;
}

const env = loadEnv(envPath);
const SENDER_PRIVATE_KEY = process.env.SENDER_PRIVATE_KEY || env.SENDER_PRIVATE_KEY;
const RPC_URL = process.env.BASE_RPC_URL || env.BASE_RPC_URL || "https://sepolia.base.org";

const to = process.argv[2];
const amountEth = process.argv[3] || "0.00001";

if (!to) {
    console.error("❌ Usage: node scripts/send_eth.mjs <stealth_address> [amount_in_eth]");
    process.exit(1);
}

if (!SENDER_PRIVATE_KEY || SENDER_PRIVATE_KEY.includes("1111111111")) {
    console.error("❌ SENDER_PRIVATE_KEY not set in .env");
    console.error("   Add the private key of a Base Sepolia funded wallet to your .env:");
    console.error("   SENDER_PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE");
    process.exit(1);
}

async function main() {
    const provider = new ethers.JsonRpcProvider(RPC_URL);
    const wallet = new ethers.Wallet(SENDER_PRIVATE_KEY, provider);

    const balance = await provider.getBalance(wallet.address);
    console.log(`📤 Sender: ${wallet.address}`);
    console.log(`💰 Balance: ${ethers.formatEther(balance)} ETH`);
    console.log(`📬 Sending ${amountEth} ETH to: ${to}`);

    const tx = await wallet.sendTransaction({
        to,
        value: ethers.parseEther(amountEth),
    });

    console.log(`⏳ Tx submitted: ${tx.hash}`);
    console.log(`   https://sepolia.basescan.org/tx/${tx.hash}`);

    const receipt = await tx.wait();
    console.log(`✅ Confirmed in block ${receipt.blockNumber}`);
}

main().catch((e) => {
    console.error(`❌ Send failed: ${e.message}`);
    process.exit(1);
});
