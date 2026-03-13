# Secrets Setup & Environment Variables

This document describes how to configure the secrets and environment variables required to run the CloakFund backend and frontend.

## Environment Variables

The system relies on a `.env` file located in the root directory (or respective service directories) to securely load credentials. **Never commit the `.env` file to version control.**

Use the provided `.env.example` as a template.

### Required Variables

| Variable Name | Description | Where to Obtain |
|---|---|---|
| `BASE_RPC_URL` | Base network HTTP RPC endpoint. | Alchemy, Infura, or public Base endpoints. |
| `BASE_WSS_URL` | Base network WebSocket RPC endpoint for the deposit watcher. | Alchemy, Infura, or public Base endpoints. |
| `CONVEX_URL` | Convex backend deployment URL. | Convex Dashboard (auto-generated via `npx convex dev`). |
| `CONVEX_ADMIN_KEY` | (Optional) Convex admin key for authenticating backend mutations. | Convex Dashboard settings. |
| `BITGO_API_KEY` | API key for the BitGo developer portal. | [BitGo Developer Portal](https://app.bitgo-test.com/) |
| `FILEVERSE_API_KEY` | API key for storing encrypted receipts on Fileverse. | Fileverse Developer Dashboard |
| `ETH_MAINNET_RPC_URL` | (Optional) Ethereum Mainnet RPC for ENS resolution. | Alchemy, Infura |

## Secrets Management

During development, store these variables in a local `.env` file. 

For production or team environments, secrets should be stored in a secure vault (e.g., AWS Secrets Manager, HashiCorp Vault, or GitHub Secrets for CI/CD) and injected into the runtime environment.

## Mocking External APIs

During the hackathon or local development, some external APIs (like BitGo or Fileverse) might be unavailable or rate-limited. 

By default, if the respective API keys are missing or a specific mock flag is set, the system will fall back to using mock endpoints located in `rust-backend/mocks/`.

### Switching from Mock to Real APIs

To use the real external integrations instead of the mocks:

1. Obtain the actual API keys from the respective providers.
2. Add the keys to your `.env` file:
   ```env
   BITGO_API_KEY=your_real_bitgo_key
   FILEVERSE_API_KEY=your_real_fileverse_key
   ```
3. Ensure any mock feature flags (e.g., `USE_MOCK_BITGO=true`) in the `.env` file are set to `false` or removed.
4. Restart the backend service.

## Rotating Keys

If a key is compromised or needs to be rotated (e.g., post-demo):
1. Generate a new key from the provider's dashboard.
2. Update the secure secrets store (vault/CI).
3. Update the local `.env` file for all developers.
4. Restart the services.
5. Invalidate/delete the old key in the provider's dashboard.