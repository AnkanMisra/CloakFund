use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use ethers::abi::{Token, encode};
use ethers::prelude::*;
use ethers::utils::keccak256;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::convex_client::ConvexRepository;

#[derive(Serialize)]
pub struct CcipResponse {
    pub data: String,
}

#[derive(Serialize)]
pub struct CcipError {
    pub message: String,
}

/// CCIP-Read Gateway Endpoint
/// GET /gateway/{sender}/{data}.json
/// Evaluates the OffchainLookup error from the CloakResolver smart contract.
pub async fn ccip_resolve(
    Path((sender, data)): Path<(String, String)>,
    State(_state): State<Arc<ConvexRepository>>,
) -> impl IntoResponse {
    // 1. Parse sender (the ENS resolver contract address)
    let resolver_addr = match sender.parse::<Address>() {
        Ok(addr) => addr,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(CcipError {
                    message: "Invalid sender address".to_string(),
                }),
            )
                .into_response();
        }
    };

    // 2. Parse data (the original request calldata `resolve(bytes,bytes)`)
    // EIP-3668 clients usually append .json to the URL.
    let calldata = match hex::decode(data.trim_start_matches("0x").trim_end_matches(".json")) {
        Ok(d) => d,
        Err(_) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(CcipError {
                    message: "Invalid calldata hex".to_string(),
                }),
            )
                .into_response();
        }
    };

    // For the Hackathon MVP, we dynamically generate a stealth address using a registered public key.
    // In a production environment, we would ABI-decode `calldata` to extract the ENS name,
    // query the blockchain for the associated `cloak.pubkey` text record, and generate the stealth address.
    let recipient_pubkey = "0x04b10912af0c04aa473bebc86f36f44eed2bbbc6bcad611287140975fafe159974b8ac6bccd806e4647e45eda540d9ae05aed61ebff5d0bff409e813d2ad33d7f6";

    let (stealth_address_str, _ephem_pub, _view_tag) =
        match crate::stealth::generate_stealth_address(recipient_pubkey) {
            Ok(res) => res,
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CcipError {
                        message: format!("Stealth generation failed: {}", e),
                    }),
                )
                    .into_response();
            }
        };

    // Parse the newly generated stealth address
    let stealth_addr = match stealth_address_str.parse::<Address>() {
        Ok(addr) => addr,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(CcipError {
                    message: "Failed to parse stealth address".to_string(),
                }),
            )
                .into_response();
        }
    };

    // The ENS `addr(bytes32)` resolution expects an ABI encoded address as the result.
    let result_data = encode(&[Token::Address(stealth_addr)]);

    // 3. Create expiration timestamp (e.g., valid for 5 minutes)
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300;

    // 4. Build the payload to sign
    // Solidity verification: keccak256(abi.encodePacked(address(this), expires, request, result))
    // encodePacked tightly packs data. Address = 20 bytes, uint64 = 8 bytes.
    let mut payload = Vec::new();
    payload.extend_from_slice(resolver_addr.as_bytes());
    payload.extend_from_slice(&expires.to_be_bytes());
    payload.extend_from_slice(&calldata);
    payload.extend_from_slice(&result_data);

    let message_hash = keccak256(&payload);

    // 5. Sign the payload using the Gateway's private key
    // Note: This must match the `signer` address registered in the CloakResolver smart contract.
    let pk_hex = std::env::var("GATEWAY_PRIVATE_KEY").unwrap_or_else(|_| {
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    });

    let wallet = match pk_hex.parse::<LocalWallet>() {
        Ok(w) => w,
        Err(_) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(CcipError {
                    message: "Invalid gateway private key configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    let signature = match wallet.sign_hash(H256::from(message_hash)) {
        Ok(s) => s,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(CcipError {
                    message: format!("Failed to sign CCIP response: {}", e),
                }),
            )
                .into_response();
        }
    };

    // 6. Encode the final response for the wallet callback
    // abi.encode(result, expires, sig)
    let sig_bytes = signature.to_vec();
    let final_response = encode(&[
        Token::Bytes(result_data),
        Token::Uint(U256::from(expires)),
        Token::Bytes(sig_bytes),
    ]);

    let response_hex = format!("0x{}", hex::encode(final_response));

    (
        axum::http::StatusCode::OK,
        Json(CcipResponse { data: response_hex }),
    )
        .into_response()
}
