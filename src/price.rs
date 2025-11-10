use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use solana_client::client_error::reqwest;

#[derive(Debug, Serialize, Deserialize)]
struct PriceInfo {
    #[serde(rename = "usdPrice")]
    pub usd_price: f64,
    #[serde(rename = "blockId")]
    pub block_id: i64,
    pub decimals: i64,
    #[serde(rename = "priceChange24h")]
    pub price_change24h: f64,
}

pub async fn get_price() -> anyhow::Result<(f64, f64)> {
    let url = "https://lite-api.jup.ag/price/v3?ids=So11111111111111111111111111111111111111112,oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";
    let resp = reqwest::get(url).await?.text().await?;
    let prices: HashMap<String, PriceInfo> = serde_json::from_str(&resp)?;
    let ore_price = prices.get("oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp");
    let sol_price = prices.get("So11111111111111111111111111111111111111112");
    if let (Some(ore), Some(sol)) = (ore_price, sol_price) {
        return Ok((ore.usd_price, sol.usd_price));
    }
    anyhow::bail!("Failed to get prices");
}
