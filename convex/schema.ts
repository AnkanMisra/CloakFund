import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  users: defineTable({
    walletAddress: v.string(),
    ensName: v.optional(v.string()),
    publicKeyHex: v.string(),
  })
    .index("by_wallet", ["walletAddress"])
    .index("by_ens", ["ensName"]),

  paylinks: defineTable({
    userId: v.optional(v.id("users")),
    ensName: v.optional(v.string()),
    recipientPublicKeyHex: v.string(),
    status: v.union(
      v.literal("active"),
      v.literal("expired"),
      v.literal("completed"),
      v.literal("cancelled"),
    ),
    metadata: v.optional(v.any()),
    chainId: v.number(),
    network: v.string(),
  })
    .index("by_status", ["status"])
    .index("by_ens_name", ["ensName"])
    .index("by_chain_id", ["chainId"]),

  ephemeralAddresses: defineTable({
    paylinkId: v.id("paylinks"),
    stealthAddress: v.string(),
    ephemeralPubkeyHex: v.string(),
    viewTag: v.number(),
    chainId: v.number(),
    network: v.string(),
    status: v.union(
      v.literal("announced"),
      v.literal("funded"),
      v.literal("swept"),
      v.literal("expired"),
    ),
  })
    .index("by_paylink", ["paylinkId"])
    .index("by_chain_and_address", ["chainId", "stealthAddress"])
    .index("by_ephemeral_pubkey", ["ephemeralPubkeyHex"])
    .index("by_status", ["status"]),

  deposits: defineTable({
    paylinkId: v.id("paylinks"),
    ephemeralAddressId: v.id("ephemeralAddresses"),

    txHash: v.string(),
    logIndex: v.optional(v.number()),
    blockNumber: v.number(),
    blockHash: v.optional(v.string()),

    fromAddress: v.string(),
    toAddress: v.string(),

    assetType: v.union(v.literal("native"), v.literal("erc20")),
    tokenAddress: v.optional(v.string()),
    amount: v.string(),
    decimals: v.optional(v.number()),
    symbol: v.optional(v.string()),

    confirmations: v.number(),
    confirmationStatus: v.union(
      v.literal("pending"),
      v.literal("confirmed"),
      v.literal("finalized"),
      v.literal("reorged"),
      v.literal("failed"),
    ),

    detectedAt: v.number(),
    confirmedAt: v.optional(v.number()),
  })
    .index("by_paylink", ["paylinkId"])
    .index("by_ephemeral_address", ["ephemeralAddressId"])
    .index("by_tx_hash", ["txHash"])
    .index("by_confirmation_status", ["confirmationStatus"])
    .index("by_block_number", ["blockNumber"]),

  receipts: defineTable({
    depositId: v.id("deposits"),
    encryptedPayload: v.string(),
    fileversePointer: v.optional(v.string()),
  }).index("by_deposit", ["depositId"]),

  sweepJobs: defineTable({
    depositId: v.id("deposits"),
    status: v.union(
      v.literal("queued"),
      v.literal("broadcasting"),
      v.literal("completed"),
      v.literal("failed"),
    ),
    sweepTxHash: v.optional(v.string()),
  })
    .index("by_deposit", ["depositId"])
    .index("by_status", ["status"]),
});
