use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepRequest {
    /// The destination address to sweep funds to
    pub address: String,
    /// Optional fee rate or other parameters can be added here
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SweepResponse {
    pub txid: String,
    pub status: Option<String>,
}

/// A client for interacting with the BitGo Express API.
#[derive(Debug, Clone)]
pub struct BitGoClient {
    base_url: String,
    access_token: String,
    client: reqwest::Client,
}

impl BitGoClient {
    /// Creates a new BitGoClient.
    /// `base_url` is typically "http://localhost:3080" for a local Express instance
    /// or the remote BitGo API URL.
    pub fn new(base_url: String, access_token: String) -> Self {
        Self {
            base_url,
            access_token,
            client: reqwest::Client::new(),
        }
    }

    /// Triggers a wallet sweep via the BitGo Express API.
    /// POST /api/v2/{coin}/wallet/{walletId}/sweep
    pub async fn sweep_wallet(
        &self,
        coin: &str,
        wallet_id: &str,
        req: SweepRequest,
    ) -> Result<SweepResponse> {
        let url = format!(
            "{}/api/v2/{}/wallet/{}/sweep",
            self.base_url.trim_end_matches('/'),
            coin,
            wallet_id
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&req)
            .send()
            .await
            .context("Failed to send sweep request to BitGo API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("BitGo API error ({}): {}", status, text);
        }

        let parsed: SweepResponse = response
            .json()
            .await
            .context("Failed to parse BitGo sweep response")?;

        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sweep_request_serialization() {
        let req = SweepRequest {
            address: "0x123".to_string(),
            fee_rate: Some(10),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"address":"0x123","feeRate":10}"#);

        let req_no_fee = SweepRequest {
            address: "0x123".to_string(),
            fee_rate: None,
        };
        let json_no_fee = serde_json::to_string(&req_no_fee).unwrap();
        assert_eq!(json_no_fee, r#"{"address":"0x123"}"#);
    }
}
