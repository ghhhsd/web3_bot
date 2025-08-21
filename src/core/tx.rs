use log::info;
use std::{env, sync::Arc, time::Duration};

use anyhow::Result;
use jito_sdk_rust::JitoJsonRpcSDK;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, VersionedTransaction},
};
use solana_system_transaction as system_transaction;
use spl_token::ui_amount_to_amount;

use solana_compute_budget_interface::ComputeBudgetInstruction;

use crate::service::jito::{get_jito_sdk, get_tip_value, wait_for_bundle_confirmation};
use crate::utils::jjj::{create_nonblocking_rpc_client, import_env_var};
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tokio::time::Instant;
use uuid::uuid;

// prioritization fee = UNIT_PRICE * UNIT_LIMIT
fn get_unit_price() -> u64 {
    env::var("UNIT_PRICE")
        .ok()
        .and_then(|v| u64::from_str(&v).ok())
        .unwrap_or(1)
}

fn get_unit_limit() -> u32 {
    env::var("UNIT_LIMIT")
        .ok()
        .and_then(|v| u32::from_str(&v).ok())
        .unwrap_or(300_000)
}

pub async fn new_signed_and_send(
    client: &RpcClient,
    keypair: &Keypair,
    mut instructions: Vec<Instruction>,
    use_jito: bool,
    uuid_string: Option<String>,
) -> Result<Vec<String>> {
    let unit_price = get_unit_price();
    let unit_limit = get_unit_limit();
    // If not using Jito, manually set the compute unit price and limit
    if !use_jito {
        let modify_compute_units = ComputeBudgetInstruction::set_compute_unit_price(unit_price);
        let add_priority_fee = ComputeBudgetInstruction::set_compute_unit_limit(unit_limit);
        instructions.insert(0, modify_compute_units);
        instructions.insert(1, add_priority_fee);
    }
    // send init tx
    let recent_blockhash = client.get_latest_blockhash()?;
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&keypair.pubkey()),
        &vec![keypair],
        recent_blockhash,
    );

    let start_time = Instant::now();
    let mut txs = vec![];
    if use_jito {
        // jito

        let jito_sdk = get_jito_sdk(uuid_string.clone());
        let jito_client = Arc::new(jito_sdk);
        let tip_account = jito_client.get_random_tip_account().await?;
        let tip_account = Pubkey::from_str(&tip_account)?;
        // let jito_client = Arc::new(JitoRpcClient::new(format!(
        //     "{}/api/v1/bundles",
        //     *jito::BLOCK_ENGINE_URL
        // )));
        let tip = get_tip_value();
        let tip_lamports = ui_amount_to_amount(tip, spl_token::native_mint::DECIMALS);
        info!(
            "tip account: {}, tip(sol): {}, lamports: {}",
            tip_account, tip, tip_lamports
        );
        // tip tx
        let transactions: Vec<VersionedTransaction> = vec![
            VersionedTransaction::from(txn),
            VersionedTransaction::from(system_transaction::transfer(
                keypair,
                &tip_account,
                tip_lamports,
                recent_blockhash,
            )),
        ];

        let bundle = json!(transactions);
        let bundle_id = jito_client
            .send_bundle(Some(bundle), uuid_string.as_deref())
            .await?;
        let bundle_id = bundle_id.to_string();
        info!("bundle_id: {}", bundle_id);

        txs = wait_for_bundle_confirmation(
            move |id: String| {
                let client = Arc::clone(&jito_client);
                async move {
                    let response = client.get_bundle_statuses(vec![id]).await;
                    let statuses = response.inspect_err(|err| {
                        info!("Error fetching bundle status: {:?}", err);
                    })?;
                    Ok(statuses)
                }
            },
            bundle_id,
            Duration::from_millis(1000),
            Duration::from_secs(10),
        )
        .await?;
    } else {
        let aaa = create_nonblocking_rpc_client().await?;
        let sig = aaa.send_transaction(&txn).await?;
        info!("signature: {:#?}", sig);
        txs.push(sig.to_string());
    }

    info!("tx ellapsed: {:?}", start_time.elapsed());
    Ok(txs)
}
