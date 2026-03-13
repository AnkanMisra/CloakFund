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

export const create = mutation({
  args: {
    userId: v.optional(v.id("users")),
    ensName: v.optional(v.string()),
    recipientPublicKeyHex: v.string(),
    metadata: v.optional(v.any()),
    chainId: v.optional(v.number()),
    network: v.optional(v.string()),
  },
  returns: v.object({
    paylinkId: v.id("paylinks"),
    userId: v.optional(v.id("users")),
    ensName: v.optional(v.string()),
    recipientPublicKeyHex: v.string(),
    status: v.string(),
    metadata: v.optional(v.any()),
    chainId: v.number(),
    network: v.string(),
  }),
  handler: async (ctx, args) => {
    const paylinkId = await ctx.db.insert("paylinks", {
      userId: args.userId,
      ensName: args.ensName,
      recipientPublicKeyHex: normalizeHex(args.recipientPublicKeyHex),
      status: "active",
      metadata: args.metadata,
      chainId: args.chainId ?? 8453,
      network: args.network ?? "base",
    });

    const paylink = await ctx.db.get(paylinkId);
    if (!paylink) {
      throw new Error("Failed to read newly created paylink");
    }

    return {
      paylinkId,
      userId: paylink.userId,
      ensName: paylink.ensName,
      recipientPublicKeyHex: paylink.recipientPublicKeyHex,
      status: paylink.status,
      metadata: paylink.metadata,
      chainId: paylink.chainId,
      network: paylink.network,
    };
  },
});

export const createEphemeralAddress = mutation({
  args: {
    paylinkId: v.id("paylinks"),
    stealthAddress: v.string(),
    ephemeralPubkeyHex: v.string(),
    viewTag: v.number(),
    chainId: v.optional(v.number()),
    network: v.optional(v.string()),
  },
  returns: v.object({
    ephemeralAddressId: v.id("ephemeralAddresses"),
    paylinkId: v.id("paylinks"),
    stealthAddress: v.string(),
    ephemeralPubkeyHex: v.string(),
    viewTag: v.number(),
    chainId: v.number(),
    network: v.string(),
    status: v.string(),
  }),
  handler: async (ctx, args) => {
    const paylink = await ctx.db.get(args.paylinkId);
    if (!paylink) {
      throw new Error("Paylink not found");
    }

    if (args.viewTag < 0 || args.viewTag > 255) {
      throw new Error("viewTag must be between 0 and 255");
    }

    const normalizedStealthAddress = normalizeAddress(args.stealthAddress);
    const normalizedEphemeralPubkey = normalizeHex(args.ephemeralPubkeyHex);
    const chainId = args.chainId ?? paylink.chainId;
    const network = args.network ?? paylink.network;

    const existing = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_chain_and_address", (q) =>
        q.eq("chainId", chainId).eq("stealthAddress", normalizedStealthAddress),
      )
      .unique();

    if (existing) {
      if (existing.paylinkId !== args.paylinkId) {
        throw new Error(
          "Stealth address already registered to a different paylink on this chain",
        );
      }
      return {
        ephemeralAddressId: existing._id,
        paylinkId: existing.paylinkId,
        stealthAddress: existing.stealthAddress,
        ephemeralPubkeyHex: existing.ephemeralPubkeyHex,
        viewTag: existing.viewTag,
        chainId: existing.chainId,
        network: existing.network,
        status: existing.status,
      };
    }

    const ephemeralAddressId = await ctx.db.insert("ephemeralAddresses", {
      paylinkId: args.paylinkId,
      stealthAddress: normalizedStealthAddress,
      ephemeralPubkeyHex: normalizedEphemeralPubkey,
      viewTag: args.viewTag,
      chainId,
      network,
      status: "announced",
    });

    return {
      ephemeralAddressId,
      paylinkId: args.paylinkId,
      stealthAddress: normalizedStealthAddress,
      ephemeralPubkeyHex: normalizedEphemeralPubkey,
      viewTag: args.viewTag,
      chainId,
      network,
      status: "announced",
    };
  },
});

export const getById = query({
  args: {
    paylinkId: v.id("paylinks"),
  },
  returns: v.union(
    v.null(),
    v.object({
      _id: v.id("paylinks"),
      _creationTime: v.number(),
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
      ephemeralAddresses: v.array(
        v.object({
          _id: v.id("ephemeralAddresses"),
          _creationTime: v.number(),
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
        }),
      ),
    }),
  ),
  handler: async (ctx, args) => {
    const paylink = await ctx.db.get(args.paylinkId);
    if (!paylink) {
      return null;
    }

    const ephemeralAddresses = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_paylink", (q) => q.eq("paylinkId", args.paylinkId))
      .collect();

    return {
      ...paylink,
      ephemeralAddresses,
    };
  },
});

export const listByEnsName = query({
  args: {
    ensName: v.string(),
  },
  returns: v.array(
    v.object({
      _id: v.id("paylinks"),
      _creationTime: v.number(),
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
    }),
  ),
  handler: async (ctx, args) => {
    return await ctx.db
      .query("paylinks")
      .withIndex("by_ens_name", (q) => q.eq("ensName", args.ensName))
      .collect();
  },
});

export const getEphemeralAddressMatch = query({
  args: {
    chainId: v.number(),
    stealthAddress: v.string(),
  },
  returns: v.union(
    v.null(),
    v.object({
      paylinkId: v.id("paylinks"),
      ephemeralAddressId: v.id("ephemeralAddresses"),
      stealthAddress: v.string(),
    }),
  ),
  handler: async (ctx, args) => {
    const normalizedStealthAddress = normalizeAddress(args.stealthAddress);

    const match = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_chain_and_address", (q) =>
        q
          .eq("chainId", args.chainId)
          .eq("stealthAddress", normalizedStealthAddress),
      )
      .unique();

    if (!match) {
      return null;
    }

    return {
      paylinkId: match.paylinkId,
      ephemeralAddressId: match._id,
      stealthAddress: match.stealthAddress,
    };
  },
});

export const markEphemeralAddressFunded = mutation({
  args: {
    ephemeralAddressId: v.id("ephemeralAddresses"),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    const existing = await ctx.db.get(args.ephemeralAddressId);
    if (!existing) {
      throw new Error("Ephemeral address not found");
    }

    if (existing.status === "announced") {
      await ctx.db.patch(args.ephemeralAddressId, {
        status: "funded",
      });
    }

    return null;
  },
});

export const updatePaylinkStatus = mutation({
  args: {
    paylinkId: v.id("paylinks"),
    status: v.union(
      v.literal("active"),
      v.literal("expired"),
      v.literal("completed"),
      v.literal("cancelled"),
    ),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    const existing = await ctx.db.get(args.paylinkId);
    if (!existing) {
      throw new Error("Paylink not found");
    }

    await ctx.db.patch(args.paylinkId, {
      status: args.status,
    });

    return null;
  },
});
