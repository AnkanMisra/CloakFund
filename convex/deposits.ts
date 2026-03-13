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

function confirmationStatusFromCount(
  confirmations: number,
  requiredConfirmations: number,
): "pending" | "confirmed" | "finalized" {
  if (confirmations < requiredConfirmations) return "pending";
  if (confirmations === requiredConfirmations) return "confirmed";
  return "finalized";
}

export const upsertDeposit = mutation({
  args: {
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

    detectedAt: v.optional(v.number()),
    confirmedAt: v.optional(v.number()),
  },
  returns: v.object({
    depositId: v.id("deposits"),
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
  }),
  handler: async (ctx, args) => {
    const paylink = await ctx.db.get(args.paylinkId);
    if (!paylink) {
      throw new Error("Paylink not found");
    }

    const ephemeralAddress = await ctx.db.get(args.ephemeralAddressId);
    if (!ephemeralAddress) {
      throw new Error("Ephemeral address not found");
    }

    if (ephemeralAddress.paylinkId !== args.paylinkId) {
      throw new Error(
        "Ephemeral address does not belong to the specified paylink",
      );
    }

    const normalizedTxHash = normalizeHex(args.txHash);
    const normalizedFromAddress = normalizeAddress(args.fromAddress);
    const normalizedToAddress = normalizeAddress(args.toAddress);
    const normalizedTokenAddress = args.tokenAddress
      ? normalizeAddress(args.tokenAddress)
      : undefined;

    const existingByTxHash = await ctx.db
      .query("deposits")
      .withIndex("by_tx_hash", (q) => q.eq("txHash", normalizedTxHash))
      .collect();

    const existing =
      existingByTxHash.find((deposit) => {
        const sameLogIndex =
          deposit.logIndex === args.logIndex ||
          (deposit.logIndex === undefined && args.logIndex === undefined);
        return (
          deposit.ephemeralAddressId === args.ephemeralAddressId &&
          sameLogIndex &&
          deposit.toAddress === normalizedToAddress
        );
      }) ??
      existingByTxHash.find(
        (deposit) =>
          deposit.ephemeralAddressId === args.ephemeralAddressId &&
          deposit.toAddress === normalizedToAddress,
      );

    const detectedAt = args.detectedAt ?? Date.now();
    const confirmedAt =
      args.confirmationStatus === "confirmed" ||
      args.confirmationStatus === "finalized"
        ? (args.confirmedAt ?? Date.now())
        : args.confirmedAt;

    if (existing) {
      await ctx.db.patch(existing._id, {
        blockNumber: args.blockNumber,
        blockHash: args.blockHash,
        fromAddress: normalizedFromAddress,
        toAddress: normalizedToAddress,
        assetType: args.assetType,
        tokenAddress: normalizedTokenAddress,
        amount: args.amount,
        decimals: args.decimals,
        symbol: args.symbol,
        confirmations: args.confirmations,
        confirmationStatus: args.confirmationStatus,
        confirmedAt,
      });

      if (
        args.confirmationStatus === "confirmed" ||
        args.confirmationStatus === "finalized"
      ) {
        if (ephemeralAddress.status === "announced") {
          await ctx.db.patch(args.ephemeralAddressId, {
            status: "funded",
          });
        }
      }

      if (
        args.confirmationStatus === "finalized" &&
        existing.confirmationStatus !== "finalized"
      ) {
        const existingJob = await ctx.db
          .query("sweepJobs")
          .withIndex("by_deposit", (q) => q.eq("depositId", existing._id))
          .first();
        if (!existingJob) {
          await ctx.db.insert("sweepJobs", {
            depositId: existing._id,
            status: "queued",
          });
        }
      }

      return {
        depositId: existing._id,
        paylinkId: existing.paylinkId,
        ephemeralAddressId: existing.ephemeralAddressId,
        txHash: normalizedTxHash,
        logIndex: args.logIndex,
        blockNumber: args.blockNumber,
        blockHash: args.blockHash,
        fromAddress: normalizedFromAddress,
        toAddress: normalizedToAddress,
        assetType: args.assetType,
        tokenAddress: normalizedTokenAddress,
        amount: args.amount,
        decimals: args.decimals,
        symbol: args.symbol,
        confirmations: args.confirmations,
        confirmationStatus: args.confirmationStatus,
        detectedAt: existing.detectedAt,
        confirmedAt,
      };
    }

    const depositId = await ctx.db.insert("deposits", {
      paylinkId: args.paylinkId,
      ephemeralAddressId: args.ephemeralAddressId,
      txHash: normalizedTxHash,
      logIndex: args.logIndex,
      blockNumber: args.blockNumber,
      blockHash: args.blockHash,
      fromAddress: normalizedFromAddress,
      toAddress: normalizedToAddress,
      assetType: args.assetType,
      tokenAddress: normalizedTokenAddress,
      amount: args.amount,
      decimals: args.decimals,
      symbol: args.symbol,
      confirmations: args.confirmations,
      confirmationStatus: args.confirmationStatus,
      detectedAt,
      confirmedAt,
    });

    if (
      args.confirmationStatus === "confirmed" ||
      args.confirmationStatus === "finalized"
    ) {
      if (ephemeralAddress.status === "announced") {
        await ctx.db.patch(args.ephemeralAddressId, {
          status: "funded",
        });
      }
    }

    if (args.confirmationStatus === "finalized") {
      await ctx.db.insert("sweepJobs", {
        depositId,
        status: "queued",
      });
    }

    return {
      depositId,
      paylinkId: args.paylinkId,
      ephemeralAddressId: args.ephemeralAddressId,
      txHash: normalizedTxHash,
      logIndex: args.logIndex,
      blockNumber: args.blockNumber,
      blockHash: args.blockHash,
      fromAddress: normalizedFromAddress,
      toAddress: normalizedToAddress,
      assetType: args.assetType,
      tokenAddress: normalizedTokenAddress,
      amount: args.amount,
      decimals: args.decimals,
      symbol: args.symbol,
      confirmations: args.confirmations,
      confirmationStatus: args.confirmationStatus,
      detectedAt,
      confirmedAt,
    };
  },
});

export const updateConfirmations = mutation({
  args: {
    depositId: v.id("deposits"),
    confirmations: v.number(),
    requiredConfirmations: v.number(),
  },
  returns: v.union(
    v.null(),
    v.object({
      depositId: v.id("deposits"),
      confirmations: v.number(),
      confirmationStatus: v.union(
        v.literal("pending"),
        v.literal("confirmed"),
        v.literal("finalized"),
        v.literal("reorged"),
        v.literal("failed"),
      ),
      confirmedAt: v.optional(v.number()),
    }),
  ),
  handler: async (ctx, args) => {
    const existing = await ctx.db.get(args.depositId);
    if (!existing) {
      return null;
    }

    if (
      existing.confirmationStatus === "reorged" ||
      existing.confirmationStatus === "failed"
    ) {
      return {
        depositId: existing._id,
        confirmations: existing.confirmations,
        confirmationStatus: existing.confirmationStatus as
          | "pending"
          | "confirmed"
          | "finalized"
          | "reorged"
          | "failed",
        confirmedAt: existing.confirmedAt,
      };
    }

    const nextStatus = confirmationStatusFromCount(
      args.confirmations,
      args.requiredConfirmations,
    );
    const confirmedAt =
      nextStatus === "confirmed" || nextStatus === "finalized"
        ? (existing.confirmedAt ?? Date.now())
        : existing.confirmedAt;

    await ctx.db.patch(args.depositId, {
      confirmations: args.confirmations,
      confirmationStatus: nextStatus,
      confirmedAt,
    });

    if (nextStatus === "confirmed" || nextStatus === "finalized") {
      const ephemeralAddress = await ctx.db.get(existing.ephemeralAddressId);
      if (ephemeralAddress && ephemeralAddress.status === "announced") {
        await ctx.db.patch(existing.ephemeralAddressId, {
          status: "funded",
        });
      }
    }

    if (
      nextStatus === "finalized" &&
      existing.confirmationStatus !== "finalized"
    ) {
      const existingJob = await ctx.db
        .query("sweepJobs")
        .withIndex("by_deposit", (q) => q.eq("depositId", existing._id))
        .first();
      if (!existingJob) {
        await ctx.db.insert("sweepJobs", {
          depositId: existing._id,
          status: "queued",
        });
      }
    }

    return {
      depositId: existing._id,
      confirmations: args.confirmations,
      confirmationStatus: nextStatus,
      confirmedAt,
    };
  },
});

export const markDepositReorged = mutation({
  args: {
    depositId: v.id("deposits"),
  },
  returns: v.null(),
  handler: async (ctx, args) => {
    const existing = await ctx.db.get(args.depositId);
    if (!existing) {
      throw new Error("Deposit not found");
    }

    await ctx.db.patch(args.depositId, {
      confirmationStatus: "reorged",
    });

    return null;
  },
});

export const getByPaylinkId = query({
  args: {
    paylinkId: v.id("paylinks"),
  },
  returns: v.array(
    v.object({
      _id: v.id("deposits"),
      _creationTime: v.number(),
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
    }),
  ),
  handler: async (ctx, args) => {
    return await ctx.db
      .query("deposits")
      .withIndex("by_paylink", (q) => q.eq("paylinkId", args.paylinkId))
      .collect();
  },
});

export const getPendingConfirmationUpdates = query({
  args: {},
  returns: v.array(
    v.object({
      _id: v.id("deposits"),
      _creationTime: v.number(),
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
    }),
  ),
  handler: async (ctx) => {
    const pending = await ctx.db
      .query("deposits")
      .withIndex("by_confirmation_status", (q) =>
        q.eq("confirmationStatus", "pending"),
      )
      .collect();

    const confirmed = await ctx.db
      .query("deposits")
      .withIndex("by_confirmation_status", (q) =>
        q.eq("confirmationStatus", "confirmed"),
      )
      .collect();

    return [...pending, ...confirmed];
  },
});

export const getDepositStatus = query({
  args: {
    paylinkId: v.id("paylinks"),
  },
  returns: v.object({
    paylinkId: v.id("paylinks"),
    deposits: v.array(
      v.object({
        depositId: v.id("deposits"),
        txHash: v.string(),
        blockNumber: v.number(),
        fromAddress: v.string(),
        toAddress: v.string(),
        assetType: v.string(),
        tokenAddress: v.optional(v.string()),
        amount: v.string(),
        decimals: v.optional(v.number()),
        symbol: v.optional(v.string()),
        confirmations: v.number(),
        confirmationStatus: v.string(),
        detectedAt: v.number(),
        confirmedAt: v.optional(v.number()),
      }),
    ),
    totalConfirmedNativeAmount: v.string(),
    totalConfirmedTokenAmounts: v.array(
      v.object({
        tokenAddress: v.string(),
        symbol: v.optional(v.string()),
        decimals: v.optional(v.number()),
        totalAmount: v.string(),
      }),
    ),
  }),
  handler: async (ctx, args) => {
    const deposits = await ctx.db
      .query("deposits")
      .withIndex("by_paylink", (q) => q.eq("paylinkId", args.paylinkId))
      .collect();

    let nativeTotal = 0n;
    const tokenTotals = new Map<
      string,
      {
        tokenAddress: string;
        symbol?: string;
        decimals?: number;
        totalAmount: bigint;
      }
    >();

    for (const deposit of deposits) {
      const isConfirmed =
        deposit.confirmationStatus === "confirmed" ||
        deposit.confirmationStatus === "finalized";

      if (!isConfirmed) continue;

      const amount = BigInt(deposit.amount);

      if (deposit.assetType === "native") {
        nativeTotal += amount;
      } else if (deposit.tokenAddress) {
        const key = deposit.tokenAddress;
        const existing = tokenTotals.get(key);
        if (existing) {
          existing.totalAmount += amount;
        } else {
          tokenTotals.set(key, {
            tokenAddress: key,
            symbol: deposit.symbol,
            decimals: deposit.decimals,
            totalAmount: amount,
          });
        }
      }
    }

    return {
      paylinkId: args.paylinkId,
      deposits: deposits.map((deposit) => ({
        depositId: deposit._id,
        txHash: deposit.txHash,
        blockNumber: deposit.blockNumber,
        fromAddress: deposit.fromAddress,
        toAddress: deposit.toAddress,
        assetType: deposit.assetType,
        tokenAddress: deposit.tokenAddress,
        amount: deposit.amount,
        decimals: deposit.decimals,
        symbol: deposit.symbol,
        confirmations: deposit.confirmations,
        confirmationStatus: deposit.confirmationStatus,
        detectedAt: deposit.detectedAt,
        confirmedAt: deposit.confirmedAt,
      })),
      totalConfirmedNativeAmount: nativeTotal.toString(),
      totalConfirmedTokenAmounts: Array.from(tokenTotals.values()).map(
        (item) => ({
          tokenAddress: item.tokenAddress,
          symbol: item.symbol,
          decimals: item.decimals,
          totalAmount: item.totalAmount.toString(),
        }),
      ),
    };
  },
});

export const getLatestCheckpoint = query({
  args: {},
  returns: v.union(
    v.null(),
    v.object({
      startBlock: v.number(),
      latestProcessedBlock: v.optional(v.number()),
      latestConfirmedBlock: v.optional(v.number()),
    }),
  ),
  handler: async (ctx) => {
    const latestDeposit = await ctx.db
      .query("deposits")
      .withIndex("by_block_number")
      .order("desc")
      .first();

    if (!latestDeposit) {
      return null;
    }

    return {
      startBlock: latestDeposit.blockNumber,
      latestProcessedBlock: latestDeposit.blockNumber,
      latestConfirmedBlock: latestDeposit.blockNumber,
    };
  },
});
