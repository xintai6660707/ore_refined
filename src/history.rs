use serde::{Deserialize, Serialize};
use solana_client::client_error::reqwest;
use solana_sdk::pubkey::Pubkey;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardHistory {
    pub disc: u8,
    pub round_id: u64,
    pub start_slot: u64,
    pub end_slot: u64,
    pub winning_square: u8,
    pub top_miner: Vec<u8>,
    pub num_winners: u32,
    pub total_deployed: u64,
    pub total_vaulted: u64,
    pub total_winnings: u64,
    pub total_minted: u64,
    pub ts: u64,
}

pub async fn get_history_winners() -> anyhow::Result<Vec<Pubkey>> {
    let resp = reqwest::get("https://ore-bsm.onrender.com/board/history")
        .await?
        .text()
        .await?;

    let arr: Vec<Vec<serde_json::Value>> = serde_json::from_str(&resp)?;

    let mut winners = vec![];
    for item in arr {
        let key_bytes: Vec<u8> = serde_json::from_value(item[0].clone())?;
        let board_info: BoardHistory = serde_json::from_value(item[1].clone())?;
        let pubkey = Pubkey::try_from(board_info.top_miner.as_slice()).unwrap();
        info!("board id: {}, winning square: {}, top miner: {}", board_info.round_id, board_info.winning_square, pubkey);
        winners.push(pubkey);
    }

    Ok(winners)
}
