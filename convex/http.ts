import { httpRouter } from "convex/server";
import { httpAction } from "./_generated/server";
import { api } from "./_generated/api";

const http = httpRouter();

function json(
  body: unknown,
  init?: ResponseInit,
): Response {
  return new Response(JSON.stringify(body), {
    status: init?.status ?? 200,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
}

http.route({
  path: "/health",
  method: "GET",
  handler: httpAction(async () => {
    return json({
      ok: true,
      service: "cloakfund-convex",
      timestamp: Date.now(),
    });
  }),
});

http.route({
  path: "/deposit-status",
  method: "GET",
  handler: httpAction(async (ctx, request) => {
    const url = new URL(request.url);
    const paylinkId = url.searchParams.get("paylinkId");

    if (!paylinkId) {
      return json(
        {
          ok: false,
          error: "Missing required query parameter: paylinkId",
        },
        { status: 400 },
      );
    }

    try {
      const result = await ctx.runQuery(api.deposits.getDepositStatus, {
        paylinkId: paylinkId as any,
      });

      return json({
        ok: true,
        data: result,
      });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unknown error";

      return json(
        {
          ok: false,
          error: "Failed to fetch deposit status",
          details: message,
        },
        { status: 500 },
      );
    }
  }),
});

export default http;
