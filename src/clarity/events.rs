use super::types::{
    Value,
    PrincipalData,
    StandardPrincipalData,
    QualifiedContractIdentifier,
    AssetIdentifier
};
use super::costs::ExecutionCost;
use super::analysis::ContractAnalysis;
#[derive(Debug, Clone, PartialEq)]
pub struct StacksTransactionReceipt {
    pub events: Vec<StacksTransactionEvent>,
    pub result: Value,
    pub stx_burned: u128,
    pub contract_analysis: Option<ContractAnalysis>,
    pub execution_cost: ExecutionCost
}

#[derive(Debug, Clone, PartialEq)]
pub enum StacksTransactionEvent {
    SmartContractEvent(SmartContractEventData),
    STXEvent(STXEventType),
    NFTEvent(NFTEventType),
    FTEvent(FTEventType),
}

impl StacksTransactionEvent {

    pub fn json_serialize(&self) -> serde_json::Value {
        match self {
            StacksTransactionEvent::SmartContractEvent(event_data) => json!({
                    "type": "contract_event",
                    "contract_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::STXEvent(STXEventType::STXTransferEvent(event_data)) => json!({
                "type": "stx_transfer_event",
                "stx_transfer_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::STXEvent(STXEventType::STXMintEvent(event_data)) => json!({
                "type": "stx_mint_event",
                "stx_mint_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::STXEvent(STXEventType::STXBurnEvent(event_data)) => json!({
                "type": "stx_burn_event",
                "stx_burn_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::NFTEvent(NFTEventType::NFTTransferEvent(event_data)) => json!({
                "type": "nft_transfer_event",
                "nft_transfer_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::NFTEvent(NFTEventType::NFTMintEvent(event_data)) => json!({
                "type": "nft_mint_event",
                "nft_mint_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::FTEvent(FTEventType::FTTransferEvent(event_data)) => json!({
                "type": "ft_transfer_event",
                "ft_transfer_event": event_data.json_serialize()
            }),
            StacksTransactionEvent::FTEvent(FTEventType::FTMintEvent(event_data)) => json!({
                "type": "ft_mint_event",
                "ft_mint_event": event_data.json_serialize()
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum STXEventType {
    STXTransferEvent(STXTransferEventData),
    STXMintEvent(STXMintEventData),
    STXBurnEvent(STXBurnEventData)
}

#[derive(Debug, Clone, PartialEq)]
pub enum NFTEventType {
    NFTTransferEvent(NFTTransferEventData),
    NFTMintEvent(NFTMintEventData),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FTEventType {
    FTTransferEvent(FTTransferEventData),
    FTMintEvent(FTMintEventData),
}

#[derive(Debug, Clone, PartialEq)]
pub struct STXTransferEventData {
    pub sender: PrincipalData,
    pub recipient: PrincipalData,
    pub amount: u128,
}

impl STXTransferEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "sender": format!("{}",self.sender),
            "recipient": format!("{}",self.recipient),
            "amount": format!("{}", self.amount),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct STXMintEventData {
    pub recipient: PrincipalData,
    pub amount: u128,
}

impl STXMintEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "recipient": format!("{}",self.recipient),
            "amount": format!("{}", self.amount),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct STXBurnEventData {
    pub sender: PrincipalData,
    pub amount: u128,
}

impl STXBurnEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "sender": format!("{}",self.sender),
            "amount": format!("{}", self.amount),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NFTTransferEventData {
    pub asset_identifier: AssetIdentifier,
    pub sender: PrincipalData,
    pub recipient: PrincipalData,
    pub value: Value,
}

impl NFTTransferEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "asset_identifier": format!("{}", self.asset_identifier),
            "sender": format!("{}",self.sender),
            "recipient": format!("{}",self.recipient),
            "value": self.value,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NFTMintEventData {
    pub asset_identifier: AssetIdentifier,
    pub recipient: PrincipalData,
    pub value: Value,
}

impl NFTMintEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "asset_identifier": format!("{}", self.asset_identifier),
            "recipient": format!("{}",self.recipient),
            "value": self.value,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FTTransferEventData {
    pub asset_identifier: AssetIdentifier,
    pub sender: PrincipalData,
    pub recipient: PrincipalData,
    pub amount: u128,
}

impl FTTransferEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "asset_identifier": format!("{}", self.asset_identifier),
            "sender": format!("{}",self.sender),
            "recipient": format!("{}",self.recipient),
            "amount": format!("{}", self.amount),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FTMintEventData {
    pub asset_identifier: AssetIdentifier,
    pub recipient: PrincipalData,
    pub amount: u128,
}

impl FTMintEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "asset_identifier": format!("{}", self.asset_identifier),
            "recipient": format!("{}",self.recipient),
            "amount": format!("{}", self.amount),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartContractEventData {
    pub key: (QualifiedContractIdentifier, String),
    pub value: Value,
}

impl SmartContractEventData {
    pub fn json_serialize(&self) -> serde_json::Value {
        json!({
            "contract_identifier": self.key.0.to_string(),
            "topic": self.key.1,
            "value": self.value,
        })
    }
}
