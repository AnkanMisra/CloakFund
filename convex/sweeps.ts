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
    })
  ),
  handler: async (ctx) => {
    return await ctx.db
      .query("sweepJobs")
      .withIndex("by_status", (q) => q.eq("status", "queued"))
      .collect();
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
