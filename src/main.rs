mod history;
mod jito;
mod onchain_main;
mod utils;
mod price;

use clap::{command, Parser, Subcommand};

use crate::history::get_history_winners;
use crate::onchain_main::get_ore_refined_ix;
use anchor_lang::declare_program;
use anchor_lang::prelude::*;
use ore_api::prelude::*;
use rand::Rng;
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_client::rpc_filter::MemcmpEncodedBytes;
use solana_client::{
    client_error::{reqwest::StatusCode, ClientErrorKind},
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Keypair;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    keccak::hash,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use spl_token::amount_to_ui_amount;
use std::str::FromStr;
use std::sync::Arc;
use solana_sdk::address_lookup_table::AddressLookupTableAccount;
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use spl_associated_token_account::get_associated_token_address;
use steel::{AccountDeserialize, Clock, Discriminator, Numeric};
use tokio::select;
use tokio::sync::Mutex;
use tracing::info;
use utils::*;
use crate::jito::send_bundle;
use crate::price::get_price;

declare_program!(ore_por_program);

pub const DEFALUT_UNITS: u64 = 400_000;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志订阅器
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    info!("Args: {:?}", args);

    // 之后的日志会被正确格式化输出
    info!("程序启动");


    let commitment = CommitmentConfig::processed();

    // Build transaction
    let rpc = Arc::new(RpcClient::new_with_commitment(
        args.rpc
            .parse()
            .unwrap(),
        commitment,
    ));

    let payer = Arc::new(read_keypair_file(args.keypair.clone()).unwrap());
    get_balance(&rpc,&payer).await?;

    on_chain_main(&rpc, &payer,args).await?;

    select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down.");
        }
    }

    Ok(())
}


async fn get_balance(
    rpc: &Arc<RpcClient>,
    payer: &Arc<Keypair>,
) -> anyhow::Result<()> {
    let mut miner = get_miner(&rpc, payer.pubkey()).await?;

    let treasury = get_treasury(&rpc).await?;
    if treasury.miner_rewards_factor > miner.rewards_factor {
        let accumulated_rewards = treasury.miner_rewards_factor - miner.rewards_factor;
        if accumulated_rewards < Numeric::ZERO {
            panic!("Accumulated rewards is negative");
        }
        let personal_rewards = accumulated_rewards * Numeric::from_u64(miner.rewards_ore);
        miner.refined_ore += personal_rewards.to_u64();
    }


    //获取sol余额
    let sol_balance = rpc.get_balance(&payer.pubkey()).await?;


    // 获取pair的ORE token地址
    let ore_ata_address = get_associated_token_address(&payer.pubkey(), &pubkey!("oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp"));
    let ore_amount = rpc.get_token_account_balance(&ore_ata_address).await?;
    let wallet_ore = ore_amount.amount.parse::<u64>().unwrap_or(0);



    info!("wallet: {:?} sol:{:.2} unclaimed_sol:{}\t \t wallet_ore:{:.2} \t unclaimed_ore: {:.2} \t refined_ore: {:.2}",
                payer.pubkey(),
                amount_to_ui_amount(sol_balance, 9),
                amount_to_ui_amount(miner.rewards_sol, 9),
                amount_to_ui_amount(wallet_ore, TOKEN_DECIMALS),
                amount_to_ui_amount(miner.rewards_ore, TOKEN_DECIMALS),
                amount_to_ui_amount(miner.refined_ore, TOKEN_DECIMALS),
    );

    Ok(())
}


async fn on_chain_main(
    rpc: &Arc<RpcClient>,
    payer: &Arc<Keypair>,
    args: Args
) -> anyhow::Result<()> {
    let board_mutex = Arc::new(Mutex::new(get_board(&rpc).await?));
    let clock_mutex = Arc::new(Mutex::new(get_clock(&rpc).await?));
    let miner_mutex = Arc::new(Mutex::new(get_miner(&rpc,payer.pubkey()).await?));
    let round_mutex = Arc::new(Mutex::new(get_round(&rpc,board_mutex.lock().await.round_id).await?));

    update_board_loop(rpc.clone(), board_mutex.clone()).await?;
    update_clock_loop(rpc.clone(), clock_mutex.clone()).await?;
    update_miner_loop(rpc.clone(), payer.clone(),miner_mutex.clone()).await?;
    update_round_loop(rpc.clone(), round_mutex.clone(),board_mutex.clone()).await?;


    let mut last_round_id = 0_u64;
    let mut req_id = 0;

    loop {
        req_id += 1;
        req_id = req_id % 100;
        // checkpoint(rpc.clone(), payer, miner_mutex.clone(), board_mutex.clone()).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        let board = board_mutex.lock().await.clone();
        let clock = clock_mutex.lock().await.clone();
        let miner = miner_mutex.lock().await.clone();
        let round_id = board.round_id;
        let (mut ore_price,mut sol_price) = get_price().await?;


        if last_round_id != round_id {
            info!("New round detected: {}", round_id);
            last_round_id = round_id;
            (ore_price,sol_price) = get_price().await?;
            info!("ORE price: {} USDC", ore_price);
            info!("SOL price: {} USDC", sol_price);
        }




        let slot_left = board.end_slot.saturating_sub(clock.slot);

        info!("round_id: {:?} slot_left: {:?}", round_id, slot_left);

        if slot_left > args.remaining_slots as u64 {
            continue;
        }

        let checkpoint_ix = checkpoint(payer.pubkey(), payer.pubkey(), miner.round_id);
        let refined_ix = get_ore_refined_ix(
            payer.pubkey(),
            round_id,
            ore_price,
            sol_price,
            (args.per_round_deploy_amount * 1e9f64) as u64,
            args.remaining_slots,
            args.ore_refined_rate,
            req_id,
        )?;
        let claim_sol_ix = claim_sol(payer.pubkey());
        let ixs = [checkpoint_ix.clone(),refined_ix.clone(),claim_sol_ix];

        // send ixs by rpc
        if slot_left > 1 {
            let simulate_result = simulate_transaction(&rpc, &payer, &ixs).await?;
            let mut units_consumed = simulate_result.value.units_consumed.unwrap_or(0);
            units_consumed = (units_consumed * 11 / 10).max(200_000);
            
            if simulate_result.value.err.is_some() {
                info!(
                    "simulate transaction failed: {:?}",
                    simulate_result.value.err
                );
                continue;
            } else {
                //send ixs by rpc
                submit_transaction_with_ixs(&rpc, &payer, &ixs, units_consumed).await?;

                //send ixs by jito
                req_id += 1;
                let rpc_clone = rpc.clone();
                let payer_clone = payer.clone();
                tokio::spawn(async move {
                    let result = send_ix_use_jito(&rpc_clone, &payer_clone, &ixs,units_consumed).await;
                });
            }
        }

        
    }

    Ok(())
}

async fn update_board_loop(
    rpc_client: Arc<RpcClient>,
    board: Arc<Mutex<Board>>,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        loop {
            // 获取新的市场数据
            let new_board = get_board(&rpc_client).await.unwrap();

            // 获取锁并更新数据
            {
                let mut board_guard = board.lock().await;
                *board_guard = new_board;
            }

            // 添加延时避免过于频繁的请求
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}

async fn update_miner_loop(
    rpc: Arc<RpcClient>,
    payer: Arc<Keypair>,
    miner: Arc<Mutex<Miner>>,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        loop {
            // 获取新的miner
            let new_miner = get_miner(&rpc, payer.pubkey()).await.unwrap();

            // 获取锁并更新数据
            {
                let mut miner_guard = miner.lock().await;
                *miner_guard = new_miner;
            }

            // 添加延时避免过于频繁的请求
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}

async fn update_clock_loop(rpc: Arc<RpcClient>, clock: Arc<Mutex<Clock>>) -> anyhow::Result<()> {
    tokio::spawn(async move {
        loop {
            // 获取新的clock
            let new_clock = get_clock(&rpc).await.unwrap();

            // 获取锁并更新数据
            {
                let mut clock_guard = clock.lock().await;
                *clock_guard = new_clock;
            }

            // 添加延时避免过于频繁的请求
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}


async fn update_round_loop(
    rpc: Arc<RpcClient>,
    round: Arc<Mutex<Round>>,
    board: Arc<Mutex<Board>>,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        loop {
            let round_id = {
                board.lock().await.round_id
            };
            // 获取新的clock
            let new_round = get_round(&rpc,round_id).await.unwrap();

            // 获取锁并更新数据
            {
                let mut clock_guard = round.lock().await;
                *clock_guard = new_round;
            }

            // 添加延时避免过于频繁的请求
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    });
    Ok(())
}



#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {

    #[arg(
        long,
        value_name = "RPC_URL",
        help = "RPC address of your RPC provider,The recommended one is Helius",
    )]
    rpc: String,

    #[arg(
        long,
        value_name = "KEYPAIR_PATH",
        help = "Path to your Solana keypair file",
    )]
    keypair: String,

    #[arg(
        long,
        value_name = "PER_ROUND_DEPLOY_AMOUNT",
        help = "The amount of SOL you expect to deploy in each round",
    )]
    per_round_deploy_amount: f64,


    #[arg(
        long,
        value_name = "REMAINING_SLOTS",
        help = "The required slot for the transaction to land, specified by the number of slots remaining in the round.",
        default_value = "15"
    )]
    remaining_slots: u8,


    #[arg(
        long,
        value_name = "ORE_REFINED_RATE",
        help = "The refined rate of ORE you expect to get when deploying SOL. e.g. 1.3 means 1.3 ORE can be refined to 1 unclaimed ORE. The minimum is 1.1.",
        default_value = "1.3"
    )]
    ore_refined_rate: f64

}