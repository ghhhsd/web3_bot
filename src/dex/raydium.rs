use crate::{
    common::{
        logger::Logger,
        utils::{SwapConfig, import_env_var},
    },
    core::{
        token::{get_account_info, get_associated_token_address, get_mint_info},
        tx,
    },
    engine::swap::{SwapDirection, SwapInType},
};

use anyhow::{Context, Result, anyhow};
use bytemuck;
use raydium_amm::accounts::AmmInfo;
use reqwest::Proxy;
use serde::Deserialize;
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::{
    instruction::Instruction, program_pack::Pack, pubkey::Pubkey, signature::Keypair,
    signer::Signer,
};

use raydium_amm::instructions::{SwapBaseIn, swap_base_in, swap_base_out, SwapBaseInInstructionArgs, SwapBaseOutInstructionArgs, SwapBaseOut};
use solana_program::system_instruction;
use spl_associated_token_account::instruction::create_associated_token_account;
use spl_token::{amount_to_ui_amount, state::Account, ui_amount_to_amount};
use spl_token_client::token::TokenError;
use std::{str::FromStr, sync::Arc};
use log::info;

pub const AMM_PROGRAM: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

#[derive(Clone, Debug, PartialEq)]
pub struct AmmSwapInfoResult {
    pub pool_id: Pubkey,
    pub amm_authority: Pubkey,
    pub amm_open_orders: Pubkey,
    pub amm_coin_vault: Pubkey,
    pub amm_pc_vault: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub market_program: Pubkey,
    pub market: Pubkey,
    pub market_coin_vault: Pubkey,
    pub market_pc_vault: Pubkey,
    pub market_vault_signer: Pubkey,
    pub market_event_queue: Pubkey,
    pub market_bids: Pubkey,
    pub market_asks: Pubkey,
    pub amount_specified: u64,
    pub other_amount_threshold: u64,
}
#[derive(Debug, Deserialize)]
pub struct PoolInfo {
    pub success: bool,
    pub data: PoolData,
}

#[derive(Debug, Deserialize)]
pub struct PoolData {
    // pub count: u32,
    pub data: Vec<Pool>,
}

impl PoolData {
    pub fn get_pool(&self) -> Option<Pool> {
        self.data.first().cloned()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Pool {
    pub id: String,
    #[serde(rename = "programId")]
    pub program_id: String,
    #[serde(rename = "mintA")]
    pub mint_a: Mint,
    #[serde(rename = "mintB")]
    pub mint_b: Mint,
    #[serde(rename = "marketId")]
    pub market_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Mint {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

pub struct Raydium {
    pub rpc_nonblocking_client: Arc<solana_client::nonblocking::rpc_client::RpcClient>,
    pub rpc_client: Option<Arc<solana_client::rpc_client::RpcClient>>,
    pub keypair: Arc<Keypair>,
    pub pool_id: Option<String>,
}

impl Raydium {
    pub fn new(
        rpc_nonblocking_client: Arc<solana_client::nonblocking::rpc_client::RpcClient>,
        rpc_client: Arc<solana_client::rpc_client::RpcClient>,
        keypair: Arc<Keypair>,
    ) -> Self {
        Self {
            rpc_nonblocking_client,
            keypair,
            rpc_client: Some(rpc_client),
            pool_id: None,
        }
    }

    pub async fn swap(
        &self,
        swap_config: SwapConfig,
        amm_pool_id: Pubkey,
        pool_state: AmmInfo,
    ) -> Result<Vec<String>> {
        info!("[SWAP IN RAYDIUM]({}) => ");
        let slippage_bps = swap_config.slippage * 100;
        let owner = self.keypair.pubkey();
        let program_id = spl_token::ID;
        let native_mint = spl_token::native_mint::ID;
        let mint = pool_state.coin_vault_mint;

        let (token_in, token_out, user_input_token, swap_base_in) = match (
            swap_config.swap_direction.clone(),
            pool_state.coin_vault_mint == native_mint,
        ) {
            (SwapDirection::Buy, true) => (native_mint, mint, pool_state.coin_vault, true),
            (SwapDirection::Buy, false) => (native_mint, mint, pool_state.pc_vault, true),
            (SwapDirection::Sell, true) => (mint, native_mint, pool_state.pc_vault, true),
            (SwapDirection::Sell, false) => (mint, native_mint, pool_state.coin_vault, true),
        };

        info!(
            "token_in:{}, token_out:{}, user_input_token:{}, swap_base_in:{}",
            token_in, token_out, user_input_token, swap_base_in
        );

        let in_ata = get_associated_token_address(
            self.rpc_nonblocking_client.clone(),
            self.keypair.clone(),
            &token_in,
            &owner,
        );
        let out_ata = get_associated_token_address(
            self.rpc_nonblocking_client.clone(),
            self.keypair.clone(),
            &token_out,
            &owner,
        );

        let mut create_instruction = None;
        let mut close_instruction = None;

        tx::new_signed_and_send(&client, &self.keypair, instructions, swap_config.use_jito).await
    }
}

pub fn amm_swap(
    amm_program: Pubkey,
    amm_pool: Pubkey,
    amm_authority: Pubkey,
    amm_open_orders: Pubkey,
    amm_coin_vault: Pubkey,
    amm_pc_vault: Pubkey,
    market_program: Pubkey,
    market: Pubkey,
    market_bids: Pubkey,
    market_asks: Pubkey,
    market_event_queue: Pubkey,
    market_coin_vault: Pubkey,
    market_pc_vault: Pubkey,
    market_vault_signer: Pubkey,
    user_token_source: Pubkey,
    user_token_destination: Pubkey,
    user_source_owner: Pubkey,
    amount_in: u64,
    amount_out: u64,
    is_swap_base_in: bool,
) -> Instruction {
    match is_swap_base_in {
        true => {
            let args = SwapBaseInInstructionArgs{
                amount_in,
                minimum_amount_out:amount_out,
            };
            let sbi = SwapBaseIn {
                token_program: amm_program,
                amm: amm_pool,
                amm_authority,
                amm_open_orders,
                amm_target_orders: Default::default(),
                pool_coin_token_account: amm_coin_vault,
                pool_pc_token_account: amm_pc_vault,
                serum_program: market_program,
                serum_market: market,
                serum_bids: market_bids,
                serum_asks: market_asks,
                serum_event_queue: market_event_queue,
                serum_coin_vault_account: market_coin_vault,
                serum_pc_vault_account: market_pc_vault,
                serum_vault_signer: market_vault_signer,
                uer_source_token_account: user_token_source,
                uer_destination_token_account: user_token_destination,
                user_source_owner: user_source_owner,
            };
            sbi.instruction_with_remaining_accounts(args,&[])
        }
        false => {
            let args = SwapBaseOutInstructionArgs{
                max_amount_in: amount_in,
                amount_out: amount_out,
            };
            let sbi = SwapBaseOut {
                token_program: amm_program,
                amm: amm_pool,
                amm_authority,
                amm_open_orders,
                amm_target_orders: Default::default(),
                pool_coin_token_account: amm_coin_vault,
                pool_pc_token_account: amm_pc_vault,
                serum_program: market_program,
                serum_market: market,
                serum_bids: market_bids,
                serum_asks: market_asks,
                serum_event_queue: market_event_queue,
                serum_coin_vault_account: market_coin_vault,
                serum_pc_vault_account: market_pc_vault,
                serum_vault_signer: market_vault_signer,
                uer_source_token_account: user_token_source,
                uer_destination_token_account: user_token_destination,
                user_source_owner: user_source_owner,
            };
            sbi.instruction_with_remaining_accounts(args,&[])
        }
    }
}

pub async fn get_pool_state(
    rpc_client: Arc<solana_client::rpc_client::RpcClient>,
    pool_id: Option<&str>,
    mint: Option<&str>,
) -> Result<(Pubkey, AmmInfo)> {
    if let Some(pool_id) = pool_id {
        info!("[FIND POOL STATE BY pool_id]: {}", pool_id);
        let amm_pool_id = Pubkey::from_str(pool_id)?;
        let pool_data = common::rpc::get_account(&rpc_client, &amm_pool_id)?
            .ok_or(anyhow!("NotFoundPool: pool state not found"))?;
        let pool_state: &AmmInfo =
            bytemuck::from_bytes(&pool_data[0..core::mem::size_of::<AmmInfo>()]);
        Ok((amm_pool_id, *pool_state))
    } else if let Some(mint) = mint {
        // find pool by mint via rpc
        if let Ok(pool_state) = get_pool_state_by_mint(rpc_client.clone(), mint, logger).await {
            return Ok(pool_state);
        }
        // find pool by mint via raydium api
        let pool_data = get_pool_info(&spl_token::native_mint::ID.to_string(), mint).await;
        if let Ok(pool_data) = pool_data {
            let pool = pool_data
                .get_pool()
                .ok_or(anyhow!("NotFoundPool: pool not found in raydium api"))?;
            let amm_pool_id = Pubkey::from_str(&pool.id)?;
            info!("[FIND POOL STATE BY raydium api]: {}", amm_pool_id);
            let pool_data = common::rpc::get_account(&rpc_client, &amm_pool_id)?
                .ok_or(anyhow!("NotFoundPool: pool state not found"))?;
            let pool_state: &AmmInfo =
                bytemuck::from_bytes(&pool_data[0..core::mem::size_of::<AmmInfo>()]);

            return Ok((amm_pool_id, *pool_state));
        }
        Err(anyhow!("NotFoundPool: pool state not found"))
    } else {
        Err(anyhow!("NotFoundPool: pool state not found"))
    }
}

pub async fn get_pool_state_by_mint(
    rpc_client: Arc<solana_client::rpc_client::RpcClient>,
    mint: &str,
) -> Result<(Pubkey, AmmInfo)> {
    info!("[FIND POOL STATE BY mint]: {}", mint);
    let pairs = vec![
        // pump pool
        (
            Some(spl_token::native_mint::ID),
            Pubkey::from_str(mint).ok(),
        ),
        // general pool
        (
            Pubkey::from_str(mint).ok(),
            Some(spl_token::native_mint::ID),
        ),
    ];

    let pool_len = core::mem::size_of::<AmmInfo>() as u64;
    let amm_program = Pubkey::from_str(AMM_PROGRAM)?;
    // Find matching AMM pool from mint pairs by filter
    let mut found_pools = None;
    for (coin_mint, pc_mint) in pairs {
        info!(
            "get_pool_state_by_mint filter: coin_mint: {:?}, pc_mint: {:?}",
            coin_mint, pc_mint
        );
        let filters = match (coin_mint, pc_mint) {
            (None, None) => Some(vec![RpcFilterType::DataSize(pool_len)]),
            (Some(coin_mint), None) => Some(vec![
                RpcFilterType::Memcmp(Memcmp::new_base58_encoded(400, &coin_mint.to_bytes())),
                RpcFilterType::DataSize(pool_len),
            ]),
            (None, Some(pc_mint)) => Some(vec![
                RpcFilterType::Memcmp(Memcmp::new_base58_encoded(432, &pc_mint.to_bytes())),
                RpcFilterType::DataSize(pool_len),
            ]),
            (Some(coin_mint), Some(pc_mint)) => Some(vec![
                RpcFilterType::Memcmp(Memcmp::new_base58_encoded(400, &coin_mint.to_bytes())),
                RpcFilterType::Memcmp(Memcmp::new_base58_encoded(432, &pc_mint.to_bytes())),
                RpcFilterType::DataSize(pool_len),
            ]),
        };
        let pools =
            common::rpc::get_program_accounts_with_filters(&rpc_client, amm_program, filters)
                .unwrap();
        if !pools.is_empty() {
            found_pools = Some(pools);
            break;
        }
    }

    match found_pools {
        Some(pools) => {
            let pool = &pools[0];
            let pool_state = AmmInfo::load_from_bytes(&pools[0].1.data)?;
            Ok((pool.0, *pool_state))
        }
        None => Err(anyhow!("NotFoundPool: pool state not found")),
    }
}

// get pool info
// https://api-v3.raydium.io/pools/info/mint?mint1=So11111111111111111111111111111111111111112&mint2=EzM2d8JVpzfhV7km3tUsR1U1S4xwkrPnWkM4QFeTpump&poolType=standard&poolSortField=default&sortType=desc&pageSize=10&page=1
pub async fn get_pool_info(mint1: &str, mint2: &str) -> Result<PoolData> {
    let mut client_builder = reqwest::Client::builder();
    let http_proxy = import_env_var("HTTP_PROXY");
    let proxy = Proxy::all(http_proxy)?;
    client_builder = client_builder.proxy(proxy);
    let client = client_builder.build()?;

    let result = client
        .get("https://api-v3.raydium.io/pools/info/mint")
        .query(&[
            ("mint1", mint1),
            ("mint2", mint2),
            ("poolType", "standard"),
            ("poolSortField", "default"),
            ("sortType", "desc"),
            ("pageSize", "1"),
            ("page", "1"),
        ])
        .send()
        .await?
        .json::<PoolInfo>()
        .await
        .context("Failed to parse pool info JSON")?;
    Ok(result.data)
}
