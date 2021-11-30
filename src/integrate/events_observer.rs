use super::DevnetEvent;
use crate::integrate::{MempoolAdmissionData, ServiceStatusData, Status};
use crate::poke::load_session;
use crate::publish::{publish_contract, Network};
use crate::types::{self, AccountConfig, DevnetConfig};
use crate::types::{
    AccountIdentifier, Amount, BitcoinBlockData, BitcoinBlockMetadata, BitcoinTransactionData,
    BitcoinTransactionMetadata, BlockIdentifier, Currency, CurrencyMetadata, CurrencyStandard,
    Operation, OperationIdentifier, OperationStatusKind, OperationType, StacksBlockData,
    StacksBlockMetadata, StacksTransactionData, StacksTransactionMetadata, TransactionIdentifier,
};
use crate::utils;
use crate::utils::stacks::{transactions, StacksRpc};
use base58::FromBase58;
use clarity_repl::clarity::codec::transaction::TransactionPayload;
use clarity_repl::clarity::codec::{StacksMessageCodec, StacksTransaction};
use clarity_repl::clarity::representations::ClarityName;
use clarity_repl::clarity::types::{
    AssetIdentifier, BuffData, SequenceData, TupleData, Value as ClarityValue,
};
use clarity_repl::clarity::util::address::AddressHashMode;
use clarity_repl::clarity::util::hash::{hex_bytes, Hash160};
use clarity_repl::repl::settings::InitialContract;
use clarity_repl::repl::Session;
use rocket::config::{Config, LogLevel};
use rocket::serde::json::{json, Json, Value as JsonValue};
use rocket::serde::Deserialize;
use rocket::State;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::io::Cursor;
use std::iter::FromIterator;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::str;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use tracing::info;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use bitcoincore_rpc::bitcoin::hashes::Hash;
use bitcoincore_rpc::bitcoin::{Block, BlockHash};

#[cfg(feature = "cli")]
use crate::runnner::deno;

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct NewBurnBlock {
    burn_block_hash: String,
    burn_block_height: u64,
    reward_slot_holders: Vec<String>,
    burn_amount: u64,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct NewBlock {
    block_height: u64,
    block_hash: String,
    burn_block_height: u64,
    burn_block_hash: String,
    parent_block_hash: String,
    index_block_hash: String,
    parent_index_block_hash: String,
    transactions: Vec<NewTransaction>,
    events: Vec<NewEvent>,
    // reward_slot_holders: Vec<String>,
    // burn_amount: u32,
}

#[derive(Deserialize)]
pub struct NewMicroBlock {
    transactions: Vec<NewTransaction>,
}

#[derive(Deserialize)]
pub struct NewTransaction {
    pub txid: String,
    pub status: String,
    pub raw_result: String,
    pub raw_tx: String,
}

#[derive(Deserialize)]
pub struct NewEvent {
    pub txid: String,
    pub committed: bool,
    pub event_index: u32,
    #[serde(rename = "type")]
    pub event_type: String,
    pub stx_transfer_event: Option<JsonValue>,
    pub stx_mint_event: Option<JsonValue>,
    pub stx_burn_event: Option<JsonValue>,
    pub stx_lock_event: Option<JsonValue>,
    pub nft_transfer_event: Option<JsonValue>,
    pub nft_mint_event: Option<JsonValue>,
    pub nft_burn_event: Option<JsonValue>,
    pub ft_transfer_event: Option<JsonValue>,
    pub ft_mint_event: Option<JsonValue>,
    pub ft_burn_event: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct STXTransferEventData {
    pub sender: String,
    pub recipient: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct STXMintEventData {
    pub recipient: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct STXLockEventData {
    pub locked_amount: String,
    pub unlock_height: u64,
    pub locked_address: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct STXBurnEventData {
    pub sender: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NFTTransferEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    #[serde(rename = "value")]
    pub asset_identifier: String,
    pub sender: String,
    pub recipient: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NFTMintEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    #[serde(rename = "value")]
    pub asset_identifier: String,
    pub recipient: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NFTBurnEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    #[serde(rename = "value")]
    pub asset_identifier: String,
    pub sender: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FTTransferEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    pub sender: String,
    pub recipient: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FTMintEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    pub recipient: String,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FTBurnEventData {
    #[serde(rename = "asset_identifier")]
    pub asset_class_identifier: String,
    pub sender: String,
    pub amount: String,
}

#[derive(Clone, Debug)]
pub struct EventObserverConfig {
    pub devnet_config: DevnetConfig,
    pub accounts: BTreeMap<String, AccountConfig>,
    pub contracts_to_deploy: VecDeque<InitialContract>,
    pub manifest_path: PathBuf,
    pub pox_info: PoxInfo,
    pub session: Session,
    pub deployer_nonce: u64,
}

#[derive(Deserialize, Debug)]
struct ContractReadonlyCall {
    okay: bool,
    result: String,
}

impl EventObserverConfig {
    pub fn new(
        devnet_config: DevnetConfig,
        manifest_path: PathBuf,
        accounts: BTreeMap<String, AccountConfig>,
    ) -> Self {
        info!("Checking contracts...");
        let session = match load_session(manifest_path.clone(), false, &Network::Devnet) {
            Ok((session, _)) => session,
            Err(e) => {
                println!("{}", e);
                std::process::exit(1);
            }
        };
        EventObserverConfig {
            devnet_config,
            accounts,
            manifest_path,
            pox_info: PoxInfo::default(),
            contracts_to_deploy: VecDeque::from_iter(
                session.settings.initial_contracts.iter().map(|c| c.clone()),
            ),
            session,
            deployer_nonce: 0,
        }
    }

    pub async fn execute_scripts(&self) {
        if self.devnet_config.execute_script.len() > 0 {
            for _cmd in self.devnet_config.execute_script.iter() {
                #[cfg(feature = "cli")]
                let _ = deno::do_run_scripts(
                    vec![_cmd.script.clone()],
                    false,
                    false,
                    false,
                    _cmd.allow_wallets,
                    _cmd.allow_write,
                    self.manifest_path.clone(),
                    Some(self.session.clone()),
                )
                .await;
            }
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PoxInfo {
    contract_id: String,
    pox_activation_threshold_ustx: u64,
    first_burnchain_block_height: u64,
    prepare_phase_block_length: u32,
    reward_phase_block_length: u32,
    reward_slots: u32,
    total_liquid_supply_ustx: u64,
    next_cycle: PoxCycle,
}

impl PoxInfo {
    pub fn default() -> PoxInfo {
        PoxInfo {
            contract_id: "ST000000000000000000002AMW42H.pox".into(),
            pox_activation_threshold_ustx: 0,
            first_burnchain_block_height: 100,
            prepare_phase_block_length: 1,
            reward_phase_block_length: 4,
            reward_slots: 8,
            total_liquid_supply_ustx: 1000000000000000,
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct PoxCycle {
    min_threshold_ustx: u64,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct AssetClassCache {
    symbol: String,
    decimals: u8,
}

pub async fn start_events_observer(
    events_config: EventObserverConfig,
    devnet_event_tx: Sender<DevnetEvent>,
    terminator_rx: Receiver<bool>,
) -> Result<(), Box<dyn Error>> {
    let _ = events_config.execute_scripts().await;

    let port = events_config.devnet_config.orchestrator_port;
    let manifest_path = events_config.manifest_path.clone();
    let rw_lock = Arc::new(RwLock::new(events_config));
    let asset_class_ids_map: HashMap<String, AssetClassCache> = HashMap::new();

    let moved_rw_lock = rw_lock.clone();
    let moved_tx = Arc::new(Mutex::new(devnet_event_tx.clone()));
    let moved_cached_asset_class_ids_map = Arc::new(RwLock::new(asset_class_ids_map));

    let config = Config {
        port: port,
        workers: 4,
        address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        keep_alive: 5,
        temp_dir: std::env::temp_dir(),
        log_level: LogLevel::Debug,
        ..Config::default()
    };

    let _ = std::thread::spawn(move || {
        let future = rocket::custom(config)
            .manage(moved_rw_lock)
            .manage(moved_tx)
            .manage(moved_cached_asset_class_ids_map)
            .mount(
                "/",
                routes![
                    handle_new_burn_block,
                    handle_new_block,
                    handle_new_microblocks,
                    handle_new_mempool_tx,
                    handle_drop_mempool_tx,
                ],
            )
            .launch();
        let rt = utils::create_basic_runtime();
        rt.block_on(future).expect("Unable to spawn event observer");
    });

    loop {
        match terminator_rx.recv() {
            Ok(true) => {
                devnet_event_tx
                    .send(DevnetEvent::info("Terminating event observer".into()))
                    .expect("Unable to terminate event observer");
                break;
            }
            Ok(false) => {
                // Restart
                devnet_event_tx
                    .send(DevnetEvent::info("Reloading contracts".into()))
                    .expect("Unable to terminate event observer");

                let session = match load_session(manifest_path.clone(), false, &Network::Devnet) {
                    Ok((session, _)) => session,
                    Err(e) => {
                        devnet_event_tx
                            .send(DevnetEvent::error(format!("Contracts invalid: {}", e)))
                            .expect("Unable to terminate event observer");
                        continue;
                    }
                };
                let contracts_to_deploy = VecDeque::from_iter(
                    session.settings.initial_contracts.iter().map(|c| c.clone()),
                );
                devnet_event_tx
                    .send(DevnetEvent::success(format!(
                        "{} contracts to deploy",
                        contracts_to_deploy.len()
                    )))
                    .expect("Unable to terminate event observer");

                if let Ok(mut config_writer) = rw_lock.write() {
                    config_writer.contracts_to_deploy = contracts_to_deploy;
                    config_writer.session = session;
                    config_writer.deployer_nonce = 0;
                }
            }
            Err(_) => {
                break;
            }
        }
    }
    Ok(())
}

#[post("/new_burn_block", format = "json", data = "<new_burn_block>")]
pub fn handle_new_burn_block(
    config: &State<Arc<RwLock<EventObserverConfig>>>,
    devnet_events_tx: &State<Arc<Mutex<Sender<DevnetEvent>>>>,
    new_burn_block: Json<NewBurnBlock>,
) -> Json<JsonValue> {
    let devnet_events_tx = devnet_events_tx.inner();

    match devnet_events_tx.lock() {
        Ok(tx) => {
            let _ = tx.send(DevnetEvent::debug(format!(
                "Bitcoin block #{} received",
                new_burn_block.burn_block_height
            )));
            let mut transactions = vec![];
            // match config.read() {
            //     Ok(config_reader) => {
            //         let node_url = format!(
            //             "http://localhost:{}",
            //             config_reader.devnet_config.bitcoin_node_rpc_port
            //         );
            //         let auth = Auth::UserPass(
            //             config_reader.devnet_config.bitcoin_node_username.clone(),
            //             config_reader.devnet_config.bitcoin_node_password.clone());
            
            //         let rpc = Client::new(&node_url, auth).unwrap();
            //         let raw_value = new_burn_block.burn_block_hash.strip_prefix("0x").unwrap();
            //         let mut bytes = hex_bytes(&raw_value).unwrap();
            //         bytes.reverse();
            //         let block_hash = BlockHash::from_slice(&bytes).unwrap();
            //         let block = rpc.get_block(&block_hash).unwrap();
                    
            //         for txdata in block.txdata.iter() {
            //             let _ = tx.send(DevnetEvent::debug(format!(
            //                 "Tx.out: {:?}", txdata.output
            //             )));
            //         }
            //     },
            //     _ => {}
            // };
        
            let _ = tx.send(DevnetEvent::ServiceStatus(ServiceStatusData {
                order: 0,
                status: Status::Green,
                name: "bitcoin-node".into(),
                comment: format!(
                    "mining blocks (chaintip = #{})",
                    new_burn_block.burn_block_height
                ),
            }));
            let _ = tx.send(DevnetEvent::BitcoinBlock(BitcoinBlockData {
                block_identifier: BlockIdentifier {
                    hash: new_burn_block.burn_block_hash.clone(),
                    index: new_burn_block.burn_block_height,
                },
                parent_block_identifier: BlockIdentifier {
                    hash: "".into(), // todo(ludo): open a PR on stacks-blockchain to get this field.
                    index: new_burn_block.burn_block_height - 1,
                },
                timestamp: 0, // todo(ludo): open a PR on stacks-blockchain to get this field.
                metadata: BitcoinBlockMetadata {},
                transactions: transactions,
            }));
            let _ = tx.send(DevnetEvent::debug(format!(
                "ACK"
            )));
        }
        _ => {}
    };

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/new_block", format = "application/json", data = "<new_block>")]
pub fn handle_new_block(
    config: &State<Arc<RwLock<EventObserverConfig>>>,
    devnet_events_tx: &State<Arc<Mutex<Sender<DevnetEvent>>>>,
    asset_class_ids_map: &State<Arc<RwLock<HashMap<String, AssetClassCache>>>>,
    mut new_block: Json<NewBlock>,
) -> Json<JsonValue> {

    let devnet_events_tx = devnet_events_tx.inner();
    let config = config.inner();

    if let Ok(tx) = devnet_events_tx.lock() {
        let _ = tx.send(DevnetEvent::ServiceStatus(ServiceStatusData {
            order: 1,
            status: Status::Green,
            name: "stacks-node".into(),
            comment: format!("mining blocks (chaintip = #{})", new_block.block_height),
        }));
        let _ = tx.send(DevnetEvent::info(format!(
            "Block #{} anchored in Bitcoin block #{} includes {} transactions",
            new_block.block_height,
            new_block.burn_block_height,
            new_block.transactions.len(),
        )));
    }

    let (
        updated_config,
        first_burnchain_block_height,
        prepare_phase_block_length,
        reward_phase_block_length,
        node_url,
    ) = if let Ok(config_reader) = config.read() {
        let node_url = format!(
            "http://localhost:{}",
            config_reader.devnet_config.stacks_node_rpc_port
        );

        if config_reader.contracts_to_deploy.len() > 0 {
            let mut updated_config = config_reader.clone();

            // How many contracts left?
            let contracts_left = updated_config.contracts_to_deploy.len();
            let tx_chaining_limit = 25;
            let blocks_required = 1 + (contracts_left / tx_chaining_limit);
            let contracts_to_deploy_in_blocks = if blocks_required == 1 {
                contracts_left
            } else {
                contracts_left / blocks_required
            };

            let mut contracts_to_deploy = vec![];

            for _ in 0..contracts_to_deploy_in_blocks {
                let contract = updated_config.contracts_to_deploy.pop_front().unwrap();
                contracts_to_deploy.push(contract);
            }

            let moved_node_url = node_url.clone();

            let mut deployers_lookup = BTreeMap::new();
            for account in updated_config.session.settings.initial_accounts.iter() {
                if account.name == "deployer" {
                    deployers_lookup.insert("*".into(), account.clone());
                }
            }
            // TODO(ludo): one day, we will get rid of this shortcut
            let mut deployers_nonces = BTreeMap::new();
            deployers_nonces.insert("deployer".to_string(), config_reader.deployer_nonce);
            updated_config.deployer_nonce += contracts_to_deploy.len() as u64;

            if let Ok(tx) = devnet_events_tx.lock() {
                let _ = tx.send(DevnetEvent::success(format!(
                    "Will broadcast {} transactions",
                    contracts_to_deploy.len()
                )));
            }

            // Move the transactions submission to another thread, the clock on that thread is ticking,
            // and blocking our stacks-node
            std::thread::spawn(move || {
                for contract in contracts_to_deploy.into_iter() {
                    match publish_contract(
                        &contract,
                        &deployers_lookup,
                        &mut deployers_nonces,
                        &moved_node_url,
                        1,
                        &Network::Devnet,
                    ) {
                        Ok((_txid, _nonce)) => {
                            // let _ = tx_clone.send(DevnetEvent::success(format!(
                            //     "Contract {} broadcasted in mempool (txid: {}, nonce: {})",
                            //     contract.name.unwrap(), txid, nonce
                            // )));
                        }
                        Err(_err) => {
                            // let _ = tx_clone.send(DevnetEvent::error(err.to_string()));
                            break;
                        }
                    }
                }
            });
            (
                Some(updated_config),
                config_reader.pox_info.first_burnchain_block_height,
                config_reader.pox_info.prepare_phase_block_length,
                config_reader.pox_info.reward_phase_block_length,
                node_url,
            )
        } else {
            (
                None,
                config_reader.pox_info.first_burnchain_block_height,
                config_reader.pox_info.prepare_phase_block_length,
                config_reader.pox_info.reward_phase_block_length,
                node_url,
            )
        }
    } else {
        (None, 0, 0, 0, "".into())
    };

    if let Some(updated_config) = updated_config {
        if let Ok(mut config_writer) = config.write() {
            *config_writer = updated_config;
        }
    }

    let pox_cycle_length: u64 = (prepare_phase_block_length + reward_phase_block_length).into();
    let current_len = new_block.burn_block_height - first_burnchain_block_height;
    let pox_cycle_id: u32 = (current_len / pox_cycle_length).try_into().unwrap();

    let mut events = vec![];
    events.append(&mut new_block.events);
    let transactions = if let Ok(mut asset_class_ids_map) = asset_class_ids_map.inner().write() {
        new_block
            .transactions
            .iter()
            .map(|t| {
                let description = get_tx_description(&t.raw_tx);
                StacksTransactionData {
                    transaction_identifier: TransactionIdentifier {
                        hash: t.txid.clone(),
                    },
                    operations: get_standardized_stacks_operations(
                        t,
                        &mut events,
                        &mut asset_class_ids_map,
                        &node_url,
                    ),
                    metadata: StacksTransactionMetadata {
                        success: t.status == "success",
                        result: get_value_description(&t.raw_result),
                        events: vec![],
                        description,
                    },
                }
            })
            .collect()
    } else {
        vec![]
    };

    if let Ok(tx) = devnet_events_tx.lock() {
        let _ = tx.send(DevnetEvent::StacksBlock(StacksBlockData {
            block_identifier: BlockIdentifier {
                hash: new_block.index_block_hash.clone(),
                index: new_block.block_height,
            },
            parent_block_identifier: BlockIdentifier {
                hash: new_block.parent_index_block_hash.clone(),
                index: new_block.block_height,
            },
            timestamp: 0,
            metadata: StacksBlockMetadata {
                bitcoin_anchor_block_identifier: BlockIdentifier {
                    hash: new_block.burn_block_hash.clone(),
                    index: new_block.burn_block_height,
                },
                bitcoin_genesis_block_identifier: BlockIdentifier {
                    hash: "".into(),
                    index: first_burnchain_block_height,
                },
                pox_cycle_index: pox_cycle_id,
                pox_cycle_length: pox_cycle_length.try_into().unwrap(),
            },
            transactions,
        }));
    }

    // Every penultimate block, we check if some stacking orders should be submitted before the next
    // cycle starts.
    if new_block.burn_block_height % pox_cycle_length == (pox_cycle_length - 2) {
        if let Ok(config_reader) = config.read() {
            // let tx_clone = tx.clone();

            let accounts = config_reader.accounts.clone();
            let mut pox_info = config_reader.pox_info.clone();

            let pox_stacking_orders = config_reader.devnet_config.pox_stacking_orders.clone();
            std::thread::spawn(move || {
                let pox_url = format!("{}/v2/pox", node_url);

                if let Ok(reponse) = reqwest::blocking::get(pox_url) {
                    if let Ok(update) = reponse.json() {
                        pox_info = update
                    }
                }

                for pox_stacking_order in pox_stacking_orders.into_iter() {
                    if pox_stacking_order.start_at_cycle == (pox_cycle_id + 1) {
                        let account = match accounts.get(&pox_stacking_order.wallet) {
                            None => continue,
                            Some(account) => account,
                        };
                        let stacks_rpc = StacksRpc::new(node_url.clone());
                        let default_fee = 1000;
                        let nonce = stacks_rpc
                            .get_nonce(account.address.to_string())
                            .expect("Unable to retrieve nonce");

                        let stx_amount =
                            pox_info.next_cycle.min_threshold_ustx * pox_stacking_order.slots;
                        let (_, _, account_secret_keu) = types::compute_addresses(
                            &account.mnemonic,
                            &account.derivation,
                            account.is_mainnet,
                        );
                        let addr_bytes = pox_stacking_order
                            .btc_address
                            .from_base58()
                            .expect("Unable to get bytes from btc address");

                        let addr_bytes = Hash160::from_bytes(&addr_bytes[1..21]).unwrap();
                        let addr_version = AddressHashMode::SerializeP2PKH;
                        let stack_stx_tx = transactions::build_contrat_call_transaction(
                            pox_info.contract_id.clone(),
                            "stack-stx".into(),
                            vec![
                                ClarityValue::UInt(stx_amount.into()),
                                ClarityValue::Tuple(
                                    TupleData::from_data(vec![
                                        (
                                            ClarityName::try_from("version".to_owned()).unwrap(),
                                            ClarityValue::buff_from_byte(addr_version as u8),
                                        ),
                                        (
                                            ClarityName::try_from("hashbytes".to_owned()).unwrap(),
                                            ClarityValue::Sequence(SequenceData::Buffer(
                                                BuffData {
                                                    data: addr_bytes.as_bytes().to_vec(),
                                                },
                                            )),
                                        ),
                                    ])
                                    .unwrap(),
                                ),
                                ClarityValue::UInt((new_block.burn_block_height - 1).into()),
                                ClarityValue::UInt(pox_stacking_order.duration.into()),
                            ],
                            nonce,
                            default_fee,
                            &hex_bytes(&account_secret_keu).unwrap(),
                        );
                        let _ = stacks_rpc
                            .post_transaction(stack_stx_tx)
                            .expect("Unable to broadcast transaction");
                    }
                }
            });
        }
    }

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post(
    "/new_microblocks",
    format = "application/json",
    data = "<new_microblock>"
)]
pub fn handle_new_microblocks(
    _config: &State<Arc<RwLock<EventObserverConfig>>>,
    devnet_events_tx: &State<Arc<Mutex<Sender<DevnetEvent>>>>,
    new_microblock: Json<NewMicroBlock>,
) -> Json<JsonValue> {
    let devnet_events_tx = devnet_events_tx.inner();

    if let Ok(tx) = devnet_events_tx.lock() {
        let _ = tx.send(DevnetEvent::info(format!(
            "Microblock received including {} transactions",
            new_microblock.transactions.len(),
        )));
    }

    // let transactions = new_block
    //     .transactions
    //     .iter()
    //     .map(|t| {
    //         let description = get_tx_description(&t.raw_tx);
    //         StacksTransactionData {
    //             transaction_identifier: TransactionIdentifier {
    //                 hash: t.txid.clone(),
    //             },
    //             metadata: {
    //                 success: t.status == "success",
    //                 result: get_value_description(&t.raw_result),
    //                 events: vec![],
    //                 description,
    //             }
    //         }
    //     })
    //     .collect();

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/new_mempool_tx", format = "application/json", data = "<raw_txs>")]
pub fn handle_new_mempool_tx(
    devnet_events_tx: &State<Arc<Mutex<Sender<DevnetEvent>>>>,
    raw_txs: Json<Vec<String>>,
) -> Json<JsonValue> {
    let decoded_transactions = raw_txs
        .iter()
        .map(|t| get_tx_description(t))
        .collect::<Vec<String>>();

    if let Ok(tx_sender) = devnet_events_tx.lock() {
        for tx in decoded_transactions.into_iter() {
            let _ = tx_sender.send(DevnetEvent::MempoolAdmission(MempoolAdmissionData { tx }));
        }
    }

    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

#[post("/drop_mempool_tx", format = "application/json")]
pub fn handle_drop_mempool_tx() -> Json<JsonValue> {
    Json(json!({
        "status": 200,
        "result": "Ok",
    }))
}

fn get_value_description(raw_value: &str) -> String {
    let raw_value = match raw_value.strip_prefix("0x") {
        Some(raw_value) => raw_value,
        _ => return raw_value.to_string(),
    };
    let value_bytes = match hex_bytes(&raw_value) {
        Ok(bytes) => bytes,
        _ => return raw_value.to_string(),
    };

    let value = match ClarityValue::consensus_deserialize(&mut Cursor::new(&value_bytes)) {
        Ok(value) => format!("{}", value),
        Err(e) => {
            println!("{:?}", e);
            return raw_value.to_string();
        }
    };
    value
}

pub fn get_tx_description(raw_tx: &str) -> String {
    let raw_tx = match raw_tx.strip_prefix("0x") {
        Some(raw_tx) => raw_tx,
        _ => return raw_tx.to_string(),
    };
    let tx_bytes = match hex_bytes(&raw_tx) {
        Ok(bytes) => bytes,
        _ => return raw_tx.to_string(),
    };
    let tx = match StacksTransaction::consensus_deserialize(&mut Cursor::new(&tx_bytes)) {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("{:?}", e);
            return raw_tx.to_string();
        }
    };
    let description = match tx.payload {
        TransactionPayload::TokenTransfer(ref addr, ref amount, ref _memo) => {
            format!(
                "transfered: {} µSTX from {} to {}",
                amount,
                tx.origin_address(),
                addr
            )
        }
        TransactionPayload::ContractCall(ref contract_call) => {
            let formatted_args = contract_call
                .function_args
                .iter()
                .map(|v| format!("{}", v))
                .collect::<Vec<String>>()
                .join(", ");
            format!(
                "invoked: {}.{}::{}({})",
                contract_call.address,
                contract_call.contract_name,
                contract_call.function_name,
                formatted_args
            )
        }
        TransactionPayload::SmartContract(ref smart_contract) => {
            format!("deployed: {}.{}", tx.origin_address(), smart_contract.name)
        }
        _ => {
            format!("coinbase")
        }
    };
    description
}

fn get_standardized_stacks_operations(
    transaction: &NewTransaction,
    events: &mut Vec<NewEvent>,
    asset_class_cache: &mut HashMap<String, AssetClassCache>,
    node_url: &str,
) -> Vec<Operation> {
    let mut operations = vec![];
    let mut operation_id = 0;
    
    let mut i = 0;
    while i < events.len() {
        if events[i].txid == transaction.txid {
            let event = events.remove(i);
            if let Some(ref event_data) = event.stx_mint_event {
                let data: STXMintEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: get_stacks_currency(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.stx_lock_event {
                let data: STXLockEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Lock,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.locked_address,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.locked_amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: get_stacks_currency(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.stx_burn_event {
                let data: STXBurnEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: get_stacks_currency(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.stx_transfer_event {
                let data: STXTransferEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id + 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: get_stacks_currency(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id - 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: get_stacks_currency(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.nft_mint_event {
                let data: NFTMintEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_non_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    &data.asset_identifier,
                    asset_class_cache,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount { value: 1, currency }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.nft_burn_event {
                let data: NFTBurnEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_non_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    &data.asset_identifier,
                    asset_class_cache,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount { value: 1, currency }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.nft_transfer_event {
                let data: NFTTransferEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_non_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    &data.asset_identifier,
                    asset_class_cache,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id + 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: 1,
                        currency: currency.clone(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id - 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount { value: 1, currency }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.ft_mint_event {
                let data: FTMintEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    asset_class_cache,
                    node_url,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency,
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.ft_burn_event {
                let data: FTBurnEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    asset_class_cache,
                    node_url,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: None,
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency,
                    }),
                    metadata: None,
                });
                operation_id += 1;
            } else if let Some(ref event_data) = event.ft_transfer_event {
                let data: FTTransferEventData =
                    serde_json::from_value(event_data.clone()).expect("Unable to decode event_data");
                let currency = get_standardized_fungible_currency_from_asset_class_id(
                    &data.asset_class_identifier,
                    asset_class_cache,
                    node_url,
                );
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id + 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Debit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.sender,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency: currency.clone(),
                    }),
                    metadata: None,
                });
                operation_id += 1;
                operations.push(Operation {
                    operation_identifier: OperationIdentifier {
                        index: operation_id,
                        network_index: None,
                    },
                    related_operations: Some(vec![OperationIdentifier {
                        index: operation_id - 1,
                        network_index: None,
                    }]),
                    type_: OperationType::Credit,
                    status: Some(OperationStatusKind::Success),
                    account: AccountIdentifier {
                        address: data.recipient,
                        sub_account: None,
                    },
                    amount: Some(Amount {
                        value: data.amount.parse::<u64>().expect("Unable to parse u64"),
                        currency,
                    }),
                    metadata: None,
                });
                operation_id += 1;
            }
        } else {
            i += 1;
        }
    }
    operations
}

fn get_stacks_currency() -> Currency {
    Currency {
        symbol: "STX".into(),
        decimals: 6,
        metadata: None,
    }
}

fn get_standardized_fungible_currency_from_asset_class_id(
    asset_class_id: &str,
    asset_class_cache: &mut HashMap<String, AssetClassCache>,
    node_url: &str,
) -> Currency {
    match asset_class_cache.get(asset_class_id) {
        None => {
            let comps = asset_class_id.split("::").collect::<Vec<&str>>();
            let principal = comps[0].split(".").collect::<Vec<&str>>();

            let get_symbol_request_url = format!(
                "{}/v2/contracts/call-read/{}/{}/get-symbol",
                node_url, principal[0], principal[1],
            );

            println!("get_standardized_fungible_currency_from_asset_class_id");

            let symbol_res: ContractReadonlyCall = reqwest::blocking::get(&get_symbol_request_url)
                .expect("Unable to retrieve account")
                .json()
                .expect("Unable to parse contract");

            let raw_value = match symbol_res.result.strip_prefix("0x") {
                Some(raw_value) => raw_value,
                _ => panic!(),
            };
            let value_bytes = match hex_bytes(&raw_value) {
                Ok(bytes) => bytes,
                _ => panic!(),
            };

            let symbol = match ClarityValue::consensus_deserialize(&mut Cursor::new(&value_bytes)) {
                Ok(value) => value.expect_result_ok().expect_u128(),
                _ => panic!(),
            };

            let get_decimals_request_url = format!(
                "{}/v2/contracts/call-read/{}/{}/get-decimals",
                node_url, principal[0], principal[1],
            );

            let decimals_res: ContractReadonlyCall =
                reqwest::blocking::get(&get_decimals_request_url)
                    .expect("Unable to retrieve account")
                    .json()
                    .expect("Unable to parse contract");

            let raw_value = match decimals_res.result.strip_prefix("0x") {
                Some(raw_value) => raw_value,
                _ => panic!(),
            };
            let value_bytes = match hex_bytes(&raw_value) {
                Ok(bytes) => bytes,
                _ => panic!(),
            };

            let value = match ClarityValue::consensus_deserialize(&mut Cursor::new(&value_bytes)) {
                Ok(value) => value.expect_result_ok().expect_u128(),
                _ => panic!(),
            };

            let entry = AssetClassCache {
                symbol: format!("{}", symbol),
                decimals: value as u8,
            };

            let currency = Currency {
                symbol: entry.symbol.clone(),
                decimals: entry.decimals.into(),
                metadata: Some(CurrencyMetadata {
                    asset_class_identifier: asset_class_id.into(),
                    asset_identifier: None,
                    standard: CurrencyStandard::Sip10,
                }),
            };

            asset_class_cache.insert(asset_class_id.into(), entry);

            currency
        }
        Some(entry) => Currency {
            symbol: entry.symbol.clone(),
            decimals: entry.decimals.into(),
            metadata: Some(CurrencyMetadata {
                asset_class_identifier: asset_class_id.into(),
                asset_identifier: None,
                standard: CurrencyStandard::Sip10,
            }),
        },
    }
}

fn get_standardized_non_fungible_currency_from_asset_class_id(
    asset_class_id: &str,
    asset_id: &str,
    asset_class_cache: &mut HashMap<String, AssetClassCache>,
) -> Currency {
    Currency {
        symbol: asset_class_id.into(),
        decimals: 0,
        metadata: Some(CurrencyMetadata {
            asset_class_identifier: asset_class_id.into(),
            asset_identifier: Some(asset_id.into()),
            standard: CurrencyStandard::Sip09,
        }),
    }
}
