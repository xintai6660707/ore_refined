use serde::{Deserialize, Serialize};
use solana_client::client_error::reqwest;
use std::collections::HashMap;
use tracing::info;

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

/// 从 Jupiter API 获取 ORE 和 SOL 的实时价格
pub async fn get_price() -> anyhow::Result<(f64, f64)> {
    let url = "https://lite-api.jup.ag/price/v3?ids=So11111111111111111111111111111111111111112,oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp";
    let resp = reqwest::get(url).await?.text().await?;
    let prices: HashMap<String, PriceInfo> = serde_json::from_str(&resp)?;

    let ore_price = prices.get("oreoU2P8bN6jkk3jbaiVxYnG1dCXcYxwhwyK9jSybcp");
    let sol_price = prices.get("So11111111111111111111111111111111111111112");

    if let (Some(ore), Some(sol)) = (ore_price, sol_price) {
        info!("Price updated - ORE: ${:.4}, SOL: ${:.2}", ore.usd_price, sol.usd_price);
        return Ok((ore.usd_price, sol.usd_price));
    }

    anyhow::bail!("Failed to get prices from Jupiter API");
}

/// 带重试机制的价格获取
pub async fn get_price_with_retry(max_retries: u32) -> anyhow::Result<(f64, f64)> {
    let mut retries = 0;

    loop {
        match get_price().await {
            Ok(prices) => return Ok(prices),
            Err(e) if retries < max_retries => {
                retries += 1;
                let wait_secs = 2u64.pow(retries - 1); // 指数退避
                info!("获取价格失败（第 {} 次），等待 {}s 后重试: {:?}", retries, wait_secs, e);
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("获取价格失败，已重试 {} 次: {:?}", max_retries, e));
            }
        }
    }
}
