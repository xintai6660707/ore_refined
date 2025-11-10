use ore_api::prelude::*;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    client_error::{reqwest::StatusCode, ClientErrorKind},
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
    rpc_response::{RpcResult, RpcSimulateTransactionResult},
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    native_token::lamports_to_sol,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::amount_to_ui_amount;
use steel::{AccountDeserialize, Clock, Discriminator};
use tracing::info;

/// 获取 Board 账户（棋盘状态）
pub async fn get_board(rpc: &RpcClient) -> Result<Board, anyhow::Error> {
    let board_pda = ore_api::state::board_pda();
    let account = rpc
        .get_account_with_commitment(&board_pda.0, CommitmentConfig::processed())
        .await?;
    let account = account
        .value
        .ok_or_else(|| anyhow::anyhow!("Board account not found"))?;
    let board = Board::try_from_bytes(&account.data)?;
    Ok(*board)
}

/// 获取 Round 账户（当前轮次）
pub async fn get_round(rpc: &RpcClient, id: u64) -> Result<Round, anyhow::Error> {
    let round_pda = ore_api::state::round_pda(id);
    let account = rpc
        .get_account_with_commitment(&round_pda.0, CommitmentConfig::processed())
        .await?;
    let account = account
        .value
        .ok_or_else(|| anyhow::anyhow!("Round account not found"))?;
    let round = Round::try_from_bytes(&account.data)?;
    Ok(*round)
}

/// 获取 Treasury 账户（金库）
pub async fn get_treasury(rpc: &RpcClient) -> Result<Treasury, anyhow::Error> {
    let treasury_pda = ore_api::state::treasury_pda();
    let account = rpc.get_account(&treasury_pda.0).await?;
    let treasury = Treasury::try_from_bytes(&account.data)?;
    Ok(*treasury)
}

/// 获取 Config 账户（配置）
pub async fn get_config(rpc: &RpcClient) -> Result<Config, anyhow::Error> {
    let config_pda = ore_api::state::config_pda();
    let account = rpc.get_account(&config_pda.0).await?;
    let config = Config::try_from_bytes(&account.data)?;
    Ok(*config)
}

/// 获取 Miner 账户（矿工）
pub async fn get_miner(rpc: &RpcClient, authority: Pubkey) -> Result<Miner, anyhow::Error> {
    let miner_pda = ore_api::state::miner_pda(authority);
    let account = rpc.get_account(&miner_pda.0).await?;
    let miner = Miner::try_from_bytes(&account.data)?;
    Ok(*miner)
}

/// 获取 Clock（链上时钟）
pub async fn get_clock(rpc: &RpcClient) -> Result<Clock, anyhow::Error> {
    let account = rpc
        .get_account_with_commitment(
            &solana_sdk::sysvar::clock::ID,
            CommitmentConfig::processed(),
        )
        .await?;
    let data = account
        .value
        .ok_or_else(|| anyhow::anyhow!("Clock account not found"))?
        .data;
    let clock = bincode::deserialize::<Clock>(&data)?;
    Ok(clock)
}

/// 模拟交易
pub async fn simulate_transaction(
    rpc: &RpcClient,
    payer: &Keypair,
    instructions: &[solana_sdk::instruction::Instruction],
) -> RpcResult<RpcSimulateTransactionResult> {
    let mut all_instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(256 * 1024),
        ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
        ComputeBudgetInstruction::set_compute_unit_price(10_000),
    ];
    all_instructions.extend_from_slice(instructions);

    let blockhash = rpc.get_latest_blockhash().await.unwrap();
    let result = rpc
        .simulate_transaction(&Transaction::new_signed_with_payer(
            &all_instructions,
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        ))
        .await;

    info!("交易模拟结果: {:?}", result);
    result
}

/// 提交交易（带重试和动态 gas）
pub async fn submit_transaction_with_ixs(
    rpc: &RpcClient,
    payer: &Keypair,
    instructions: &[solana_sdk::instruction::Instruction],
    units: u64,
) -> Result<solana_sdk::signature::Signature, anyhow::Error> {
    let compute_unit_price: u64 = std::env::var("COMPUTE_UNIT_PRICE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20_000); // 默认优先费

    let max_retries = 4;
    let mut retry_count = 0;

    loop {
        let blockhash = match rpc.get_latest_blockhash().await {
            Ok(bh) => bh,
            Err(_) if retry_count < max_retries => {
                retry_count += 1;
                let wait_secs = 2u64.pow(retry_count - 1);
                info!(
                    "获取 blockhash 失败（第 {} 次），等待 {}s 后重试...",
                    retry_count, wait_secs
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
                continue;
            }
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "获取 blockhash 失败，已重试 {} 次: {:?}",
                    max_retries,
                    e
                ));
            }
        };

        let mut all_instructions = vec![
            ComputeBudgetInstruction::set_compute_unit_limit((units * 11 / 10) as u32),
            ComputeBudgetInstruction::set_compute_unit_price(compute_unit_price),
        ];
        all_instructions.extend_from_slice(instructions);

        let transaction = Transaction::new_signed_with_payer(
            &all_instructions,
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        );

        let config = solana_client::rpc_config::RpcSendTransactionConfig {
            skip_preflight: true,
            ..Default::default()
        };

        match rpc.send_transaction_with_config(&transaction, config).await {
            Ok(signature) => {
                info!("✅ 交易成功提交: {}", signature);
                return Ok(signature);
            }
            Err(e) => {
                let err_str = e.to_string().to_lowercase();
                let is_retryable = err_str.contains("blockhash")
                    || err_str.contains("timeout")
                    || err_str.contains("connection");

                if is_retryable && retry_count < max_retries {
                    retry_count += 1;
                    let wait_secs = 2u64.pow(retry_count - 1);
                    info!("交易提交失败（第 {} 次），等待 {}s 后重试: {:?}", retry_count, wait_secs, e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
                    continue;
                } else {
                    info!("❌ 交易提交失败: {:?}", e);
                    return Err(e.into());
                }
            }
        }
    }
}

/// 查询程序账户
pub async fn get_program_accounts<T>(
    client: &RpcClient,
    program_id: Pubkey,
    filters: Vec<RpcFilterType>,
) -> Result<Vec<(Pubkey, T)>, anyhow::Error>
where
    T: AccountDeserialize + Discriminator + Clone,
{
    let mut all_filters = vec![RpcFilterType::Memcmp(Memcmp::new_base58_encoded(
        0,
        &T::discriminator().to_le_bytes(),
    ))];
    all_filters.extend(filters);

    let result = client
        .get_program_accounts_with_config(
            &program_id,
            RpcProgramAccountsConfig {
                filters: Some(all_filters),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await;

    match result {
        Ok(accounts) => {
            let accounts = accounts
                .into_iter()
                .filter_map(|(pubkey, account)| {
                    if let Ok(account) = T::try_from_bytes(&account.data) {
                        Some((pubkey, account.clone()))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(accounts)
        }
        Err(err) => match err.kind {
            ClientErrorKind::Reqwest(err) => {
                if let Some(status_code) = err.status() {
                    if status_code == StatusCode::GONE {
                        panic!(
                            "\n{} Your RPC provider does not support getProgramAccounts\n",
                            "ERROR"
                        );
                    }
                }
                Err(anyhow::anyhow!("Failed to get program accounts: {}", err))
            }
            _ => Err(anyhow::anyhow!("Failed to get program accounts: {}", err)),
        },
    }
}

/// 显示余额信息
pub async fn log_balance(
    rpc: &RpcClient,
    payer: &Keypair,
) -> Result<(), anyhow::Error> {
    let mut miner = get_miner(rpc, payer.pubkey()).await?;
    let treasury = get_treasury(rpc).await?;

    // 计算累计奖励
    if treasury.miner_rewards_factor > miner.rewards_factor {
        let accumulated_rewards = treasury.miner_rewards_factor - miner.rewards_factor;
        if accumulated_rewards >= ore_api::prelude::Numeric::ZERO {
            let personal_rewards = accumulated_rewards * ore_api::prelude::Numeric::from_u64(miner.rewards_ore);
            miner.refined_ore += personal_rewards.to_u64();
        }
    }

    let sol_balance = rpc.get_balance(&payer.pubkey()).await?;
    let ore_ata = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &ore_api::prelude::MINT_ADDRESS,
    );
    let wallet_ore = match rpc.get_token_account_balance(&ore_ata).await {
        Ok(balance) => balance.amount.parse::<u64>().unwrap_or(0),
        Err(_) => 0,
    };

    info!("┌─────────────────────────────────────────────────────┐");
    info!("│ 💰 账户余额                                         │");
    info!("├─────────────────────────────────────────────────────┤");
    info!("│ 钱包: {}                  │", payer.pubkey());
    info!("│ SOL 余额: {:.6} SOL                              │", lamports_to_sol(sol_balance));
    info!("│ 未领取 SOL: {:.6} SOL                           │", lamports_to_sol(miner.rewards_sol));
    info!("│ 钱包 ORE: {:.2} ORE                               │", amount_to_ui_amount(wallet_ore, TOKEN_DECIMALS));
    info!("│ 未领取 ORE: {:.2} ORE                            │", amount_to_ui_amount(miner.rewards_ore, TOKEN_DECIMALS));
    info!("│ Refined ORE: {:.2} ORE                           │", amount_to_ui_amount(miner.refined_ore, TOKEN_DECIMALS));
    info!("└─────────────────────────────────────────────────────┘");

    Ok(())
}
