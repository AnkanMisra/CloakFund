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

type PaylinkRow = {
  revoked?: boolean;
  expiresAt?: number;
};

type UsabilityCheck =
  | { ok: true }
  | { ok: false; reason: "revoked" | "expired" };

/**
 * Checks whether a paylink may still accept deposits / register new ephemeral addresses.
 *
 * `revoked` and `expiresAt` are both optional on the stored document — legacy rows
 * persisted before this feature will have `undefined` for both, and are treated as
 * indefinitely usable (matches prior behavior).
 */
export function isPaylinkUsable(paylink: PaylinkRow, nowMs: number): UsabilityCheck {
  if (paylink.revoked === true) {
    return { ok: false, reason: "revoked" };
  }
  if (typeof paylink.expiresAt === "number" && paylink.expiresAt <= nowMs) {
    return { ok: false, reason: "expired" };
  }
  return { ok: true };
}

export const create = mutation({
  args: {
    userId: v.optional(v.id("users")),
    ensName: v.optional(v.string()),
    recipientPublicKeyHex: v.string(),
    metadata: v.optional(v.any()),
    chainId: v.optional(v.number()),
    network: v.optional(v.string()),
    expiresAt: v.optional(v.number()),
    revocationTokenHash: v.optional(v.string()),
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
    expiresAt: v.optional(v.number()),
    revoked: v.boolean(),
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
      expiresAt: args.expiresAt,
      revoked: false,
      revocationTokenHash: args.revocationTokenHash,
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
      expiresAt: paylink.expiresAt,
      revoked: paylink.revoked ?? false,
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

    const usability = isPaylinkUsable(paylink, Date.now());
    if (!usability.ok) {
      throw new Error(`Paylink is ${usability.reason}`);
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

export const createWithEphemeralAddress = mutation({
  args: {
    userId: v.optional(v.id("users")),
    ensName: v.optional(v.string()),
    recipientPublicKeyHex: v.string(),
    metadata: v.optional(v.any()),
    chainId: v.optional(v.number()),
    network: v.optional(v.string()),
    stealthAddress: v.string(),
    ephemeralPubkeyHex: v.string(),
    viewTag: v.number(),
    expiresAt: v.optional(v.number()),
    revocationTokenHash: v.optional(v.string()),
  },
  returns: v.object({
    paylinkId: v.id("paylinks"),
    ephemeralAddressId: v.id("ephemeralAddresses"),
    stealthAddress: v.string(),
    ephemeralPubkeyHex: v.string(),
    expiresAt: v.optional(v.number()),
  }),
  handler: async (ctx, args) => {
    if (args.viewTag < 0 || args.viewTag > 255) {
      throw new Error("viewTag must be between 0 and 255");
    }

    const normalizedStealthAddress = normalizeAddress(args.stealthAddress);
    const normalizedEphemeralPubkey = normalizeHex(args.ephemeralPubkeyHex);
    const chainId = args.chainId ?? 8453;
    const network = args.network ?? "base";

    const existing = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_chain_and_address", (q) =>
        q.eq("chainId", chainId).eq("stealthAddress", normalizedStealthAddress),
      )
      .unique();

    if (existing) {
      throw new Error("Stealth address already registered");
    }

    const paylinkId = await ctx.db.insert("paylinks", {
      userId: args.userId,
      ensName: args.ensName,
      recipientPublicKeyHex: normalizeHex(args.recipientPublicKeyHex),
      status: "active",
      metadata: args.metadata,
      chainId,
      network,
      expiresAt: args.expiresAt,
      revoked: false,
      revocationTokenHash: args.revocationTokenHash,
    });

    const ephemeralAddressId = await ctx.db.insert("ephemeralAddresses", {
      paylinkId,
      stealthAddress: normalizedStealthAddress,
      ephemeralPubkeyHex: normalizedEphemeralPubkey,
      viewTag: args.viewTag,
      chainId,
      network,
      status: "announced",
    });

    return {
      paylinkId,
      ephemeralAddressId,
      stealthAddress: normalizedStealthAddress,
      ephemeralPubkeyHex: normalizedEphemeralPubkey,
      expiresAt: args.expiresAt,
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
      expiresAt: v.optional(v.number()),
      revoked: v.boolean(),
      usable: v.boolean(),
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

    const revoked = paylink.revoked ?? false;
    const usable = isPaylinkUsable(paylink, Date.now()).ok;

    // Intentionally omit `revocationTokenHash` from the public view — it is
    // only consulted server-side by the `revoke` mutation.
    return {
      _id: paylink._id,
      _creationTime: paylink._creationTime,
      userId: paylink.userId,
      ensName: paylink.ensName,
      recipientPublicKeyHex: paylink.recipientPublicKeyHex,
      status: paylink.status,
      metadata: paylink.metadata,
      chainId: paylink.chainId,
      network: paylink.network,
      expiresAt: paylink.expiresAt,
      revoked,
      usable,
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
      expiresAt: v.optional(v.number()),
      revoked: v.boolean(),
      usable: v.boolean(),
    }),
  ),
  handler: async (ctx, args) => {
    const rows = await ctx.db
      .query("paylinks")
      .withIndex("by_ens_name", (q) => q.eq("ensName", args.ensName))
      .collect();
    const now = Date.now();
    return rows.map((p) => ({
      _id: p._id,
      _creationTime: p._creationTime,
      userId: p.userId,
      ensName: p.ensName,
      recipientPublicKeyHex: p.recipientPublicKeyHex,
      status: p.status,
      metadata: p.metadata,
      chainId: p.chainId,
      network: p.network,
      expiresAt: p.expiresAt,
      revoked: p.revoked ?? false,
      usable: isPaylinkUsable(p, now).ok,
    }));
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

    const paylink = await ctx.db.get(match.paylinkId);
    if (!paylink || !isPaylinkUsable(paylink, Date.now()).ok) {
      return null;
    }

    return {
      paylinkId: match.paylinkId,
      ephemeralAddressId: match._id,
      stealthAddress: match.stealthAddress,
    };
  },
});

export const getActiveStealthAddresses = query({
  args: {
    chainId: v.number(),
  },
  returns: v.array(
    v.object({
      paylinkId: v.id("paylinks"),
      ephemeralAddressId: v.id("ephemeralAddresses"),
      stealthAddress: v.string(),
    }),
  ),
  handler: async (ctx, args) => {
    const addresses = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_status", (q) => q.eq("status", "announced"))
      .collect();

    const filtered = addresses.filter((a) => a.chainId === args.chainId);
    const now = Date.now();

    // Fetch every unique referenced paylink in parallel (one round-trip per
    // unique id). Previously this was a sequential await inside the loop,
    // which was O(K) round-trips where K is the number of unique paylinks.
    const uniqueIds = Array.from(
      new Set(filtered.map((a) => a.paylinkId as unknown as string)),
    );
    const fetched = await Promise.all(
      uniqueIds.map((id) => ctx.db.get(id as typeof filtered[number]["paylinkId"])),
    );
    const paylinkCache = new Map<string, PaylinkRow | null>();
    uniqueIds.forEach((id, i) => paylinkCache.set(id, fetched[i] ?? null));

    return filtered
      .filter((a) => {
        const paylink = paylinkCache.get(a.paylinkId as unknown as string);
        return paylink != null && isPaylinkUsable(paylink, now).ok;
      })
      .map((a) => ({
        paylinkId: a.paylinkId,
        ephemeralAddressId: a._id,
        stealthAddress: a.stealthAddress,
      }));
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

/**
 * Revokes a paylink by presenting the hex-encoded sha256(revocationToken) that
 * was stored at creation time. The caller does the hashing (typically the Rust
 * API), so the plaintext token never touches Convex.
 *
 * On success:
 *   - paylink.revoked is set to true
 *   - paylink.status becomes "cancelled"
 *   - every associated ephemeral address transitions to "expired" so that the
 *     watcher stops polling for deposits on their stealth addresses.
 */
export const revoke = mutation({
  args: {
    paylinkId: v.id("paylinks"),
    revocationTokenHash: v.string(),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    const paylink = await ctx.db.get(args.paylinkId);
    if (!paylink) {
      throw new Error("Paylink not found");
    }

    // Check revoked FIRST — before the token comparison — so that an
    // already-revoked paylink returns the same 409 regardless of whether the
    // caller holds the correct token. This removes a token-confirmation
    // oracle where a correct token → 409 and a wrong token → 403 would have
    // leaked whether the caller has the right token for a revoked paylink.
    if (paylink.revoked === true) {
      throw new Error("Paylink is already revoked");
    }

    // Unify "not revocable" and "wrong token" under the same client-facing
    // error so an attacker cannot distinguish a paylink that has no
    // revocation hash from one that has a different hash. Both map to 403.
    if (!paylink.revocationTokenHash) {
      throw new Error("Invalid revocation token");
    }

    const presented = args.revocationTokenHash.trim().toLowerCase();
    const stored = paylink.revocationTokenHash.trim().toLowerCase();
    if (presented.length !== stored.length || presented !== stored) {
      throw new Error("Invalid revocation token");
    }

    await ctx.db.patch(args.paylinkId, {
      revoked: true,
      status: "cancelled",
    });

    const ephems = await ctx.db
      .query("ephemeralAddresses")
      .withIndex("by_paylink", (q) => q.eq("paylinkId", args.paylinkId))
      .collect();
    await Promise.all(
      ephems
        .filter((e) => e.status !== "swept" && e.status !== "expired")
        .map((e) => ctx.db.patch(e._id, { status: "expired" })),
    );

    return null;
  },
});
