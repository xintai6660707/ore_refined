use std::collections::HashSet;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;
use ore_api::consts::MINT_ADDRESS;
// use ore_api::prelude::{block_pda, config_pda, market_pda, miner_pda, vault_address, Block, Config, Market, Miner, SwapDirection, SwapPrecision};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::pubkey::Pubkey;
use solana_program::{system_program, sysvar};
use anchor_client::{
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, signature::Keypair,
        signer::Signer,
    },
    Client, Cluster,
};
use anchor_lang::prelude::*;
use log::info;
use ore_api::prelude::{automation_pda, board_pda, miner_pda, treasury_pda};
use ore_api::state::round_pda;
use spl_associated_token_account::get_associated_token_address;

declare_program!(ore_por_program);
use ore_por_program::{ client::accounts, client::args};

pub fn get_ore_refined_ix(
    signer: Pubkey,
    round_id: u64,
    ore_price: f64,
    sol_price: f64,
    deploy_amount: u64,
    remaining_slots: u8,
    ore_refined_rate: f64,
    req_id: u8,
) -> anyhow::Result<Instruction> {

    info!("deploy_amount: {:?}",deploy_amount);


    let url = Cluster::Custom(
        "http://localhost:8899".to_string(),
        "ws://127.0.0.1:8900".to_string(),
    );

    let payer = Arc::new(Keypair::new());
    let program_client = Client::new(url.clone(), payer.clone());
    // Create program client
    let provider = Client::new_with_options(
        Cluster::Localnet,
        Rc::new(payer),
        CommitmentConfig::confirmed(),
    );
    let program = provider.program(ore_por_program::ID)?;

    let accounts = accounts::Refined {
        signer,
        authority: signer,
        automation: automation_pda(signer).0,
        board: board_pda().0,
        miner: miner_pda(signer).0,
        round: round_pda(round_id).0,
        treasury: treasury_pda().0,
        system_program: system_program::ID,
        ore_program: pubkey!("oreV3EG1i9BEgiAJ8b177Z2S2rMarzak4NMv1kULvWv"),
        fee: pubkey!("Feei2iwqp9Adcyte1F5XnKzGTFL1VDg4VyiypvoeiJyJ")
    };




    let swap_ix = program.request()
        .accounts(accounts)
        // Remaining accounts
        .args(args::Refined{
            ore_price,
            sol_price,
            amount: deploy_amount,
            remaining_slots,
            ore_refined_rate,
            req_id
        })
        .instructions()?.remove(0);


    Ok(swap_ix)
}