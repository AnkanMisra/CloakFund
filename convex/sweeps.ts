import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

export const getPendingSweepJobs = query({
  args: {},
  returns: v.array(
    v.object({
      _id: v.id("sweepJobs"),
      _creationTime: v.number(),
      depositId: v.id("deposits"),
      status: v.union(
        v.literal("queued"),
        v.literal("broadcasting"),
        v.literal("completed"),
        v.literal("failed"),
      ),
      sweepTxHash: v.optional(v.string()),
      destinationAddress: v.optional(v.string()),
      stealthAddress: v.string(),
      ephemeralPubkeyHex: v.string(),
      amount: v.string(),
      assetType: v.string(),
      tokenAddress: v.optional(v.string()),
    }),
  ),
  handler: async (ctx) => {
    const jobs = await ctx.db
      .query("sweepJobs")
      .withIndex("by_status", (q) => q.eq("status", "queued"))
      .collect();

    const result = [];
    for (const job of jobs) {
      const deposit = await ctx.db.get(job.depositId);
      if (!deposit) continue;

      const ephemeralAddress = await ctx.db.get(deposit.ephemeralAddressId);
      if (!ephemeralAddress) continue;

      result.push({
        ...job,
        stealthAddress: ephemeralAddress.stealthAddress,
        ephemeralPubkeyHex: ephemeralAddress.ephemeralPubkeyHex,
        amount: deposit.amount,
        assetType: deposit.assetType,
        tokenAddress: deposit.tokenAddress,
      });
    }

    return result;
  },
});

export const updateSweepJobStatus = mutation({
  args: {
    jobId: v.id("sweepJobs"),
    status: v.union(
      v.literal("queued"),
      v.literal("broadcasting"),
      v.literal("completed"),
      v.literal("failed"),
    ),
    sweepTxHash: v.optional(v.string()),
    destinationAddress: v.optional(v.string()),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    const existing = await ctx.db.get(args.jobId);
    if (!existing) {
      throw new Error("Sweep job not found");
    }

    const updateFields: any = { status: args.status };
    if (args.sweepTxHash !== undefined) {
      updateFields.sweepTxHash = args.sweepTxHash;
    }
    if (args.destinationAddress !== undefined) {
      updateFields.destinationAddress = args.destinationAddress;
    }

    await ctx.db.patch(args.jobId, updateFields);

    if (args.status === "completed") {
      const deposit = await ctx.db.get(existing.depositId);
      if (deposit && deposit.ephemeralAddressId) {
        const ephemeralAddress = await ctx.db.get(deposit.ephemeralAddressId);
        if (ephemeralAddress && ephemeralAddress.status !== "swept") {
          await ctx.db.patch(deposit.ephemeralAddressId, {
            status: "swept",
          });
        }
      }
    }

    return null;
  },
});

export const createSweepJob = mutation({
  args: {
    depositId: v.id("deposits"),
  },
  returns: v.id("sweepJobs"),
  handler: async (ctx, args) => {
    const deposit = await ctx.db.get(args.depositId);
    if (!deposit) {
      throw new Error("Deposit not found");
    }
    if (
      deposit.confirmationStatus !== "confirmed" &&
      deposit.confirmationStatus !== "finalized"
    ) {
      throw new Error("Deposit is not confirmed");
    }

    const existing = await ctx.db
      .query("sweepJobs")
      .withIndex("by_deposit", (q) => q.eq("depositId", args.depositId))
      .first();

    if (existing) {
      return existing._id;
    }

    return await ctx.db.insert("sweepJobs", {
      depositId: args.depositId,
      status: "queued",
    });
  },
});

export const getAllSweepJobs = query({
  args: {},
  returns: v.array(
    v.object({
      _id: v.id("sweepJobs"),
      _creationTime: v.number(),
      depositId: v.id("deposits"),
      status: v.union(
        v.literal("queued"),
        v.literal("broadcasting"),
        v.literal("completed"),
        v.literal("failed"),
      ),
      sweepTxHash: v.optional(v.string()),
      destinationAddress: v.optional(v.string()),
    }),
  ),
  handler: async (ctx) => {
    return await ctx.db.query("sweepJobs").order("desc").collect();
  },
});

/**
 * Reset any jobs stuck in "broadcasting" status back to "queued".
 * Called on sweeper startup to recover from crashes that left jobs orphaned.
 */
export const resetStuckJobs = mutation({
  args: {},
  returns: v.number(),
  handler: async (ctx) => {
    const stuckJobs = await ctx.db
      .query("sweepJobs")
      .withIndex("by_status", (q) => q.eq("status", "broadcasting"))
      .collect();

    for (const job of stuckJobs) {
      await ctx.db.patch(job._id, { status: "queued" });
    }

    return stuckJobs.length;
  },
});
