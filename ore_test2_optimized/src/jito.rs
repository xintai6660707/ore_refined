use bincode::serialize;
use rand::{seq::SliceRandom, Rng};
use serde::{de, Deserialize};
use serde_json::{json, Value};
use solana_client::client_error::reqwest;
use solana_sdk::{pubkey, pubkey::Pubkey, transaction::VersionedTransaction};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct JitoResponse<T> {
    pub result: T,
}

/// Jito 接收小费的8个地址
pub const JITO_RECIPIENTS: [Pubkey; 8] = [
    pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
    pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
];

/// 随机选择一个 Jito 小费接收地址
pub fn pick_jito_recipient() -> &'static Pubkey {
    &JITO_RECIPIENTS[rand::thread_rng().gen_range(0..JITO_RECIPIENTS.len())]
}

/// 发送 Jito 请求
async fn make_jito_request<T>(
    method: &'static str,
    block_url: &str,
    params: Value,
) -> eyre::Result<T>
where
    T: de::DeserializeOwned,
{
    let response = reqwest::Client::new()
        .post(block_url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        }))
        .send()
        .await;

    let response = match response {
        Ok(response) => response,
        Err(err) => eyre::bail!("fail to send request: {err}"),
    };

    let status = response.status();
    let text = match response.text().await {
        Ok(text) => text,
        Err(err) => eyre::bail!("fail to read response content: {err:#}"),
    };

    if !status.is_success() {
        eyre::bail!("status code: {status}, response: {text}");
    }

    let response: T = match serde_json::from_str(&text) {
        Ok(response) => response,
        Err(err) => {
            eyre::bail!("fail to deserialize response: {err:#}, response: {text}, status: {status}")
        }
    };

    Ok(response)
}

/// 发送 Bundle 到 Jito
pub async fn send_bundle(bundle: Vec<VersionedTransaction>) -> anyhow::Result<()> {
    let signature = *bundle
        .first()
        .expect("empty bundle")
        .signatures
        .first()
        .expect("empty transaction");

    let bundle = bundle
        .into_iter()
        .map(|tx| {
            let serialized = serialize(&tx).unwrap();
            solana_sdk::bs58::encode(serialized).into_string()
        })
        .collect::<Vec<_>>();

    // Jito 的5个区域端点
    let urls = [
        "https://amsterdam.mainnet.block-engine.jito.wtf/api/v1/bundles",
        "https://frankfurt.mainnet.block-engine.jito.wtf/api/v1/bundles",
        "https://ny.mainnet.block-engine.jito.wtf/api/v1/bundles",
        "https://slc.mainnet.block-engine.jito.wtf/api/v1/bundles",
        "https://tokyo.mainnet.block-engine.jito.wtf/api/v1/bundles",
    ];

    let url = urls
        .choose(&mut rand::thread_rng())
        .expect("no URLs available");

    info!("发送 Jito Bundle 到: {}", url);

    let result =
        make_jito_request::<JitoResponse<String>>("sendBundle", url, json!([bundle])).await;

    match result {
        Ok(_response) => {
            info!("✅ Jito Bundle 发送成功！签名: {}", signature);
        }
        Err(e) => {
            if e.to_string()
                .contains("bundle contains an already processed transaction")
            {
                info!("✅ Jito Bundle 已处理（交易已上链）");
            } else {
                info!("⚠️ Jito Bundle 发送失败: {:?}", e);
            }
        }
    };
    Ok(())
}

/// 构建 Jito 小费指令
pub fn build_bribe_ix(pubkey: &Pubkey, value: u64) -> solana_sdk::instruction::Instruction {
    solana_sdk::system_instruction::transfer(pubkey, pick_jito_recipient(), value)
}
