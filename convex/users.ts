import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

function normalizeHex(value: string): string {
  const trimmed = value.trim();
  return trimmed.startsWith("0x") || trimmed.startsWith("0X")
    ? `0x${trimmed.slice(2).toLowerCase()}`
    : `0x${trimmed.toLowerCase()}`;
}

function normalizeAddress(value: string): string {
  return normalizeHex(value);
}

export const upsertUser = mutation({
  args: {
    walletAddress: v.string(),
    ensName: v.optional(v.string()),
    publicKeyHex: v.string(),
  },
  returns: v.id("users"),
  handler: async (ctx, args) => {
    const normalizedWallet = normalizeAddress(args.walletAddress);
    const normalizedPubKey = normalizeHex(args.publicKeyHex);

    const existingUser = await ctx.db
      .query("users")
      .withIndex("by_wallet", (q) => q.eq("walletAddress", normalizedWallet))
      .unique();

    if (existingUser) {
      await ctx.db.patch(existingUser._id, {
        ensName: args.ensName !== undefined ? args.ensName : existingUser.ensName,
        publicKeyHex: normalizedPubKey,
      });
      return existingUser._id;
    }

    const userId = await ctx.db.insert("users", {
      walletAddress: normalizedWallet,
      ensName: args.ensName,
      publicKeyHex: normalizedPubKey,
    });

    return userId;
  },
});

export const getUserByWallet = query({
  args: {
    walletAddress: v.string(),
  },
  returns: v.union(
    v.null(),
    v.object({
      _id: v.id("users"),
      _creationTime: v.number(),
      walletAddress: v.string(),
      ensName: v.optional(v.string()),
      publicKeyHex: v.string(),
    })
  ),
  handler: async (ctx, args) => {
    const normalizedWallet = normalizeAddress(args.walletAddress);
    return await ctx.db
      .query("users")
      .withIndex("by_wallet", (q) => q.eq("walletAddress", normalizedWallet))
      .unique();
  },
});

export const getUserByEns = query({
  args: {
    ensName: v.string(),
  },
  returns: v.union(
    v.null(),
    v.object({
      _id: v.id("users"),
      _creationTime: v.number(),
      walletAddress: v.string(),
      ensName: v.optional(v.string()),
      publicKeyHex: v.string(),
    })
  ),
  handler: async (ctx, args) => {
    return await ctx.db
      .query("users")
      .withIndex("by_ens", (q) => q.eq("ensName", args.ensName))
      .first();
  },
});
