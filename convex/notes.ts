import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

/**
 * Store a privacy note (secret + nullifier + commitment) for a deposit.
 * Called by the Rust sweeper after depositing funds into the PrivacyPool contract.
 */
export const createPrivacyNote = mutation({
  args: {
    depositId: v.id("deposits"),
    sweepJobId: v.id("sweepJobs"),
    secretHex: v.string(),
    nullifierHex: v.string(),
    commitmentHex: v.string(),
    poolDepositTxHash: v.optional(v.string()),
  },
  returns: v.id("privacyNotes"),
  handler: async (ctx, args) => {
    // Check if a note already exists for this deposit
    const existing = await ctx.db
      .query("privacyNotes")
      .withIndex("by_deposit", (q) => q.eq("depositId", args.depositId))
      .first();

    if (existing) {
      // Update the existing note (e.g., to add the pool deposit tx hash)
      await ctx.db.patch(existing._id, {
        poolDepositTxHash: args.poolDepositTxHash,
      });
      return existing._id;
    }

    return await ctx.db.insert("privacyNotes", {
      depositId: args.depositId,
      sweepJobId: args.sweepJobId,
      secretHex: args.secretHex,
      nullifierHex: args.nullifierHex,
      commitmentHex: args.commitmentHex,
      poolDepositTxHash: args.poolDepositTxHash,
    });
  },
});

/**
 * Get a privacy note by deposit ID.
 * Used by the receiver to retrieve their secret note for withdrawal.
 */
export const getNoteByDeposit = query({
  args: {
    depositId: v.id("deposits"),
  },
  returns: v.union(
    v.object({
      _id: v.id("privacyNotes"),
      _creationTime: v.number(),
      depositId: v.id("deposits"),
      sweepJobId: v.id("sweepJobs"),
      secretHex: v.string(),
      nullifierHex: v.string(),
      commitmentHex: v.string(),
      poolDepositTxHash: v.optional(v.string()),
    }),
    v.null(),
  ),
  handler: async (ctx, args) => {
    return await ctx.db
      .query("privacyNotes")
      .withIndex("by_deposit", (q) => q.eq("depositId", args.depositId))
      .first();
  },
});

/**
 * Get a privacy note by commitment hash.
 * Used to verify that a commitment exists before attempting withdrawal.
 */
export const getNoteByCommitment = query({
  args: {
    commitmentHex: v.string(),
  },
  returns: v.union(
    v.object({
      _id: v.id("privacyNotes"),
      _creationTime: v.number(),
      depositId: v.id("deposits"),
      sweepJobId: v.id("sweepJobs"),
      secretHex: v.string(),
      nullifierHex: v.string(),
      commitmentHex: v.string(),
      poolDepositTxHash: v.optional(v.string()),
    }),
    v.null(),
  ),
  handler: async (ctx, args) => {
    return await ctx.db
      .query("privacyNotes")
      .withIndex("by_commitment", (q) =>
        q.eq("commitmentHex", args.commitmentHex),
      )
      .first();
  },
});

/**
 * Get all privacy notes (admin/debug).
 */
export const getAllNotes = query({
  args: {},
  returns: v.array(
    v.object({
      _id: v.id("privacyNotes"),
      _creationTime: v.number(),
      depositId: v.id("deposits"),
      sweepJobId: v.id("sweepJobs"),
      secretHex: v.string(),
      nullifierHex: v.string(),
      commitmentHex: v.string(),
      poolDepositTxHash: v.optional(v.string()),
    }),
  ),
  handler: async (ctx) => {
    return await ctx.db.query("privacyNotes").order("desc").collect();
  },
});

/**
 * Update the pool deposit tx hash for a privacy note.
 * Called after the deposit transaction is confirmed on-chain.
 */
export const updatePoolTxHash = mutation({
  args: {
    noteId: v.id("privacyNotes"),
    poolDepositTxHash: v.string(),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    await ctx.db.patch(args.noteId, {
      poolDepositTxHash: args.poolDepositTxHash,
    });
    return null;
  },
});
