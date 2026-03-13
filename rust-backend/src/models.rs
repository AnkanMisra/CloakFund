use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaylinkStatus {
    Active,
    Expired,
    Completed,
    Cancelled,
}

impl PaylinkStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EphemeralAddressStatus {
    Announced,
    Funded,
    Swept,
    Expired,
}

impl EphemeralAddressStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Announced => "announced",
            Self::Funded => "funded",
            Self::Swept => "swept",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Native,
    Erc20,
}

impl AssetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Erc20 => "erc20",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    Pending,
    Confirmed,
    Finalized,
    Reorged,
    Failed,
}

impl ConfirmationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Finalized => "finalized",
            Self::Reorged => "reorged",
            Self::Failed => "failed",
        }
    }

    pub fn from_confirmations(confirmations: u64, required_confirmations: u64) -> Self {
        if confirmations < required_confirmations {
            Self::Pending
        } else if confirmations == required_confirmations {
            Self::Confirmed
        } else {
            Self::Finalized
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaylinkRecord {
    pub id: String,
    pub creation_time: u64,
    pub ens_name: Option<String>,
    pub recipient_public_key_hex: String,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub chain_id: u64,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EphemeralAddressRecord {
    pub id: String,
    pub creation_time: u64,
    pub paylink_id: String,
    pub stealth_address: String,
    pub ephemeral_pubkey_hex: String,
    pub view_tag: u8,
    pub chain_id: u64,
    pub network: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRecord {
    pub id: String,
    pub creation_time: u64,
    pub paylink_id: String,
    pub ephemeral_address_id: String,
    pub tx_hash: String,
    pub log_index: Option<u64>,
    pub block_number: u64,
    pub block_hash: Option<String>,
    pub from_address: String,
    pub to_address: String,
    pub asset_type: String,
    pub token_address: Option<String>,
    pub amount: String,
    pub decimals: Option<u32>,
    pub symbol: Option<String>,
    pub confirmations: u64,
    pub confirmation_status: String,
    pub detected_at: u64,
    pub confirmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPaylink {
    pub ens_name: Option<String>,
    pub recipient_public_key_hex: String,
    pub metadata: Option<serde_json::Value>,
    pub chain_id: u64,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEphemeralAddress {
    pub paylink_id: String,
    pub stealth_address: String,
    pub ephemeral_pubkey_hex: String,
    pub view_tag: u8,
    pub chain_id: u64,
    pub network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDeposit {
    pub paylink_id: String,
    pub ephemeral_address_id: String,
    pub tx_hash: String,
    pub log_index: Option<u64>,
    pub block_number: u64,
    pub block_hash: Option<String>,
    pub from_address: String,
    pub to_address: String,
    pub asset_type: AssetType,
    pub token_address: Option<String>,
    pub amount: String,
    pub decimals: Option<u32>,
    pub symbol: Option<String>,
    pub confirmations: u64,
    pub confirmation_status: ConfirmationStatus,
    pub detected_at: Option<u64>,
    pub confirmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositMatch {
    pub paylink_id: String,
    pub ephemeral_address_id: String,
    pub stealth_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositView {
    pub deposit_id: String,
    pub tx_hash: String,
    pub block_number: u64,
    pub from_address: String,
    pub to_address: String,
    pub asset_type: String,
    pub token_address: Option<String>,
    pub amount: String,
    pub decimals: Option<u32>,
    pub symbol: Option<String>,
    pub confirmations: u64,
    pub confirmation_status: String,
    pub detected_at: u64,
    pub confirmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAmountSummary {
    pub token_address: String,
    pub symbol: Option<String>,
    pub decimals: Option<u32>,
    pub total_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositStatusResponse {
    pub paylink_id: String,
    pub deposits: Vec<DepositView>,
    pub total_confirmed_native_amount: String,
    pub total_confirmed_token_amounts: Vec<TokenAmountSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherCheckpoint {
    pub start_block: u64,
    pub latest_processed_block: Option<u64>,
    pub latest_confirmed_block: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherDepositEvent {
    pub tx_hash: String,
    pub log_index: Option<u64>,
    pub block_number: u64,
    pub block_hash: Option<String>,
    pub from_address: String,
    pub to_address: String,
    pub asset_type: AssetType,
    pub token_address: Option<String>,
    pub amount: String,
    pub decimals: Option<u32>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexFunctionRequest<T> {
    pub path: String,
    pub args: T,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexFunctionSuccess<T> {
    pub status: String,
    pub value: T,
    #[serde(default)]
    pub log_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexFunctionError {
    pub status: String,
    pub error_message: Option<String>,
    pub error_data: Option<serde_json::Value>,
    #[serde(default)]
    pub log_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConvexFunctionResponse<T> {
    Success(ConvexFunctionSuccess<T>),
    Error(ConvexFunctionError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertDepositResult {
    pub deposit_id: String,
    pub paylink_id: String,
    pub ephemeral_address_id: String,
    pub tx_hash: String,
    pub log_index: Option<u64>,
    pub block_number: u64,
    pub block_hash: Option<String>,
    pub from_address: String,
    pub to_address: String,
    pub asset_type: String,
    pub token_address: Option<String>,
    pub amount: String,
    pub decimals: Option<u32>,
    pub symbol: Option<String>,
    pub confirmations: u64,
    pub confirmation_status: String,
    pub detected_at: u64,
    pub confirmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationUpdateResult {
    pub deposit_id: String,
    pub confirmations: u64,
    pub confirmation_status: String,
    pub confirmed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub ok: bool,
    pub error: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositStatusApiResponse {
    pub ok: bool,
    pub data: DepositStatusResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaylinkIdParam {
    pub paylink_id: String,
}

impl DepositRecord {
    pub fn is_confirmed(&self) -> bool {
        matches!(self.confirmation_status.as_str(), "confirmed" | "finalized")
    }

    pub fn is_finalized(&self) -> bool {
        self.confirmation_status == "finalized"
    }
}

impl NewDeposit {
    pub fn normalized_confirmation_status(&self) -> &'static str {
        self.confirmation_status.as_str()
    }

    pub fn normalized_asset_type(&self) -> &'static str {
        self.asset_type.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_status_progression_behaves_as_expected() {
        assert_eq!(
            ConfirmationStatus::from_confirmations(0, 6),
            ConfirmationStatus::Pending
        );
        assert_eq!(
            ConfirmationStatus::from_confirmations(3, 6),
            ConfirmationStatus::Pending
        );
        assert_eq!(
            ConfirmationStatus::from_confirmations(6, 6),
            ConfirmationStatus::Confirmed
        );
        assert_eq!(
            ConfirmationStatus::from_confirmations(7, 6),
            ConfirmationStatus::Finalized
        );
    }

    #[test]
    fn asset_type_string_values_are_stable() {
        assert_eq!(AssetType::Native.as_str(), "native");
        assert_eq!(AssetType::Erc20.as_str(), "erc20");
    }

    #[test]
    fn deposit_record_confirmation_helpers_work() {
        let confirmed = DepositRecord {
            id: "dep1".into(),
            creation_time: 1,
            paylink_id: "pay1".into(),
            ephemeral_address_id: "ephem1".into(),
            tx_hash: "0xabc".into(),
            log_index: None,
            block_number: 10,
            block_hash: None,
            from_address: "0xfrom".into(),
            to_address: "0xto".into(),
            asset_type: "native".into(),
            token_address: None,
            amount: "100".into(),
            decimals: Some(18),
            symbol: Some("ETH".into()),
            confirmations: 6,
            confirmation_status: "confirmed".into(),
            detected_at: 1,
            confirmed_at: Some(2),
        };

        let finalized = DepositRecord {
            confirmation_status: "finalized".into(),
            ..confirmed.clone()
        };

        let pending = DepositRecord {
            confirmation_status: "pending".into(),
            ..confirmed.clone()
        };

        assert!(confirmed.is_confirmed());
        assert!(finalized.is_confirmed());
        assert!(finalized.is_finalized());
        assert!(!pending.is_confirmed());
        assert!(!pending.is_finalized());
    }
}
