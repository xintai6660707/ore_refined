use std::str::FromStr;
use std::sync::Arc;
use anchor_lang::pubkey;
use log::info;
use ore_api::prelude::*;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    client_error::{reqwest::StatusCode, ClientErrorKind},
    nonblocking::rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_client::rpc_response::{RpcResult, RpcSimulateTransactionResult};
use solana_program::address_lookup_table::AddressLookupTableAccount;
use solana_program::instruction::Instruction;
use solana_program::slot_hashes::SlotHashes;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    keccak::hash,
    pubkey::Pubkey,
    signature::{read_keypair_file, Signer},
    transaction::Transaction,
};
use solana_sdk::message::{v0, VersionedMessage};
use solana_sdk::native_token::lamports_to_sol;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::transaction::VersionedTransaction;
use spl_associated_token_account::get_associated_token_address;
use spl_token::amount_to_ui_amount;
use steel::{AccountDeserialize, Clock, Discriminator};
use crate::{jito, DEFALUT_UNITS};
use crate::jito::send_bundle;

pub async fn get_board(rpc: &RpcClient) -> Result<Board, anyhow::Error> {
    let board_pda = ore_api::state::board_pda();
    let account = rpc.get_account(&board_pda.0).await?;
    let board = Board::try_from_bytes(&account.data)?;
    Ok(*board)
}


pub async fn get_round(rpc: &RpcClient, id: u64) -> Result<Round, anyhow::Error> {
    let round_pda = ore_api::state::round_pda(id);
    let account = rpc.get_account(&round_pda.0).await?;
    let round = Round::try_from_bytes(&account.data)?;
    Ok(*round)
}

pub async fn get_treasury(rpc: &RpcClient) -> Result<Treasury, anyhow::Error> {
    let treasury_pda = ore_api::state::treasury_pda();
    let account = rpc.get_account(&treasury_pda.0).await?;
    let treasury = Treasury::try_from_bytes(&account.data)?;
    Ok(*treasury)
}

pub async fn get_config(rpc: &RpcClient) -> Result<Config, anyhow::Error> {
    let config_pda = ore_api::state::config_pda();
    let account = rpc.get_account(&config_pda.0).await?;
    let config = Config::try_from_bytes(&account.data)?;
    Ok(*config)
}

pub async fn get_miner(rpc: &RpcClient, authority: Pubkey) -> Result<Miner, anyhow::Error> {
    let miner_pda = ore_api::state::miner_pda(authority);
    let account = rpc.get_account(&miner_pda.0).await?;
    let miner = Miner::try_from_bytes(&account.data)?;
    Ok(*miner)
}

pub async fn get_clock(rpc: &RpcClient) -> Result<Clock, anyhow::Error> {
    let data = rpc.get_account_data(&solana_sdk::sysvar::clock::ID).await?;
    let clock = bincode::deserialize::<Clock>(&data)?;
    Ok(clock)
}

pub async fn claim(
    rpc: &RpcClient,
    payer: &solana_sdk::signer::keypair::Keypair,
) -> Result<(), anyhow::Error> {
    // let ix = ore_api::sdk::claim_ore(payer.pubkey(), u64::MAX);
    let ix2 = ore_api::sdk::claim_sol(payer.pubkey());
    submit_transaction_with_ixs(rpc, payer, &[ix2],DEFALUT_UNITS).await?;
    Ok(())
}

pub async fn simulate_transaction(
    rpc: &RpcClient,
    payer: &solana_sdk::signer::keypair::Keypair,
    instructions: &[solana_sdk::instruction::Instruction],
) -> RpcResult<RpcSimulateTransactionResult> {
    let mut all_instructions = vec![
        ComputeBudgetInstruction::request_heap_frame(256 * 1024),
        ComputeBudgetInstruction::set_compute_unit_limit(1000_000),
        ComputeBudgetInstruction::set_compute_unit_price(10_000),
    ];
    all_instructions.extend_from_slice(instructions);

    let blockhash = rpc.get_latest_blockhash().await.unwrap();
    let x = rpc
        .simulate_transaction(&Transaction::new_signed_with_payer(
            &all_instructions,
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        ))
        .await;
    info!("Simulation result: {:?}", x);

    return x
}


pub async fn build_multi_sign_transaction(
    rpc: &RpcClient,
    payer: &solana_sdk::signer::keypair::Keypair,
    pairs: &Vec<Arc<Keypair>>,
    instructions: &[solana_sdk::instruction::Instruction],
    address_lookup_table_accounts: &Vec<AddressLookupTableAccount>
) -> anyhow::Result<VersionedTransaction> {
    let blockhash = rpc.get_latest_blockhash().await?;
    let mut all_instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(700_000),
        ComputeBudgetInstruction::set_compute_unit_price(500_000),
    ];
    all_instructions.extend_from_slice(instructions);


    info!("Building transaction with {} signers", pairs.len()+1);
    info!("Transaction has {} instructions", all_instructions.len());
    let transaction = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &payer.pubkey(),
            &all_instructions,
            address_lookup_table_accounts,
            blockhash.clone(),
        ).unwrap()),
        pairs,
    ).unwrap();


    Ok(transaction)
}
pub async fn submit_transaction_with_ixs(
    rpc: &RpcClient,
    payer: &solana_sdk::signer::keypair::Keypair,
    instructions: &[solana_sdk::instruction::Instruction],
    units: u64,
) -> Result<(), anyhow::Error> {
    let blockhash = rpc.get_latest_blockhash().await?;
    let mut all_instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_limit((units * 11 / 10)as u32 ),
        ComputeBudgetInstruction::set_compute_unit_price(20000),
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
    rpc.send_transaction_with_config(&transaction,config).await?;
    info!("Transaction sent: {}", transaction.signatures[0]);
    Ok(())
}

pub async fn send_ix_use_jito(
    rpc: &RpcClient,
    payer: &solana_sdk::signer::keypair::Keypair,
    instructions: &[solana_sdk::instruction::Instruction],
    units: u64,
) -> anyhow::Result<()> {

    let blockhash = rpc.get_latest_blockhash().await?;
    let mut all_instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(units as u32),
        ComputeBudgetInstruction::set_compute_unit_price(0),
    ];
    all_instructions.extend_from_slice(instructions);

    let jito_ixs = [jito::build_bribe_ix(&payer.pubkey(), 5000)];
    let transaction = VersionedTransaction::try_new(
        VersionedMessage::V0(v0::Message::try_compile(
            &payer.pubkey(),
            &vec![jito_ixs.as_slice(), &all_instructions].concat(),
            &vec![],
            blockhash.clone(),
        ).unwrap()),
        &[&payer],
    ).unwrap();


    send_bundle(vec![transaction]).await?;

    Ok(())

}


pub(crate) async fn get_program_accounts<T>(
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
                            "\n{} Your RPC provider does not support the getProgramAccounts endpoint, needed to execute this command. Please use a different RPC provider.\n",
                            "ERROR"
                        );
                    }
                }
                return Err(anyhow::anyhow!("Failed to get program accounts: {}", err));
            }
            _ => return Err(anyhow::anyhow!("Failed to get program accounts: {}", err)),
        },
    }
}


pub async fn log_treasury(rpc: &RpcClient) -> Result<(), anyhow::Error> {
    let treasury_address = ore_api::state::treasury_pda().0;
    let treasury = get_treasury(rpc).await?;
    println!("Treasury");
    println!("  address: {}", treasury_address);
    println!("  balance: {} SOL", lamports_to_sol(treasury.balance));
    println!(
        "  motherlode: {} ORE",
        amount_to_ui_amount(treasury.motherlode, TOKEN_DECIMALS)
    );
    println!(
        "  miner_rewards_factor: {}",
        treasury.miner_rewards_factor.to_i80f48().to_string()
    );
    println!(
        "  stake_rewards_factor: {}",
        treasury.stake_rewards_factor.to_i80f48().to_string()
    );
    println!(
        "  total_staked: {} ORE",
        amount_to_ui_amount(treasury.total_staked, TOKEN_DECIMALS)
    );
    println!(
        "  total_unclaimed: {} ORE",
        amount_to_ui_amount(treasury.total_unclaimed, TOKEN_DECIMALS)
    );
    println!(
        "  total_refined: {} ORE",
        amount_to_ui_amount(treasury.total_refined, TOKEN_DECIMALS)
    );
    Ok(())
}