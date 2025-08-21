use crate::utils::jjj::{import_env_var, import_env_var_with_default};
use anyhow::{Result, anyhow};
use jito_sdk_rust::JitoJsonRpcSDK;
use log::{error, info, warn};
use serde_json::Value;
use tokio::time::Instant;
use tokio::time::{Duration, sleep};

pub fn get_jito_sdk(uuid_string: Option<String>) -> JitoJsonRpcSDK {
    // "https://mainnet.block-engine.jito.wtf/api/v1"
    let base_api_url = import_env_var("JITO_BLOCK_ENGINE_URL") + "/api/v1";
    JitoJsonRpcSDK::new(&base_api_url, uuid_string)
}

pub fn get_tip_value() -> f64 {
    // jito tip, the default limit is 0.1
    import_env_var_with_default("JITO_TIP_VALUE", 0.1)
}

pub async fn wait_for_bundle_confirmation<F, Fut>(
    fn1: F,
    bundle_id: String,
    period: Duration,
    period_time: Duration,
) -> Result<Vec<String>>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    let start_time = Instant::now();
    while start_time.elapsed() < period_time {
        if let Ok(res) = fn1(bundle_id.clone()).await {
            let bundle_status = get_bundle_status(&res)?;
            match bundle_status.confirmation_status.as_deref() {
                Some("confirmed") => {
                    info!("Bundle confirmed on-chain. Waiting for finalization...");
                    check_transaction_error(&bundle_status)?;
                }
                Some("finalized") => {
                    info!("Bundle finalized on-chain successfully!");
                    check_transaction_error(&bundle_status)?;
                    print_transaction_url(&bundle_status);
                    return Ok(bundle_status.transactions.unwrap_or_default());
                }
                Some(status) => {
                    warn!(
                        "Unexpected final bundle status: {}. Continuing to poll...",
                        status
                    );
                }
                None => {
                    warn!("Unable to parse final bundle status. Continuing to poll...");
                }
            }

            sleep(period).await;
        }
    }
    Err(anyhow!(
        "Failed to get finalized status after {} secs",
        period_time.as_secs()
    ))
}

fn get_bundle_status(status_response: &Value) -> Result<BundleStatus> {
    status_response
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(|value| value.as_array())
        .and_then(|statuses| statuses.first())
        .ok_or_else(|| anyhow!("Failed to parse bundle status"))
        .map(|bundle_status| BundleStatus {
            confirmation_status: bundle_status
                .get("confirmation_status")
                .and_then(|s| s.as_str())
                .map(String::from),
            err: bundle_status.get("err").cloned(),
            transactions: bundle_status
                .get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
        })
}

fn check_transaction_error(bundle_status: &BundleStatus) -> Result<()> {
    if let Some(err) = &bundle_status.err {
        if err["Ok"].is_null() {
            info!("Transaction executed without errors.");
            Ok(())
        } else {
            error!("Transaction encountered an error: {:?}", err);
            Err(anyhow!("Transaction encountered an error"))
        }
    } else {
        Ok(())
    }
}

fn print_transaction_url(bundle_status: &BundleStatus) {
    if let Some(transactions) = &bundle_status.transactions {
        if let Some(tx_id) = transactions.first() {
            info!("Transaction URL: https://solscan.io/tx/{}", tx_id);
        } else {
            warn!("Unable to extract transaction ID.");
        }
    } else {
        warn!("No transactions found in the bundle status.");
    }
}

#[derive(Debug)]
struct BundleStatus {
    confirmation_status: Option<String>,
    err: Option<Value>,
    transactions: Option<Vec<String>>,
}
