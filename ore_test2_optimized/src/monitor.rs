use crate::utils::{get_board, get_clock, get_miner, get_round};
use ore_api::prelude::{Board, Miner, Round};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::Arc;
use steel::Clock;
use tokio::sync::Mutex;
use tracing::info;

/// 实时监控系统
pub struct Monitor {
    pub board: Arc<Mutex<Board>>,
    pub clock: Arc<Mutex<Clock>>,
    pub miner: Arc<Mutex<Miner>>,
    pub round: Arc<Mutex<Round>>,
}

impl Monitor {
    /// 创建新的监控实例
    pub async fn new(rpc: &Arc<RpcClient>, payer: &Arc<Keypair>) -> Result<Self, anyhow::Error> {
        let board = get_board(rpc).await?;
        let clock = get_clock(rpc).await?;
        let miner = get_miner(rpc, payer.pubkey()).await?;
        let round = get_round(rpc, board.round_id).await?;

        Ok(Self {
            board: Arc::new(Mutex::new(board)),
            clock: Arc::new(Mutex::new(clock)),
            miner: Arc::new(Mutex::new(miner)),
            round: Arc::new(Mutex::new(round)),
        })
    }

    /// 启动所有监控循环
    pub async fn start_all(
        rpc: Arc<RpcClient>,
        payer: Arc<Keypair>,
        monitor: Arc<Self>,
    ) -> Result<(), anyhow::Error> {
        // 启动 Board 监控
        let rpc_clone = rpc.clone();
        let board_clone = monitor.board.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(new_board) = get_board(&rpc_clone).await {
                    let mut board_guard = board_clone.lock().await;
                    *board_guard = new_board;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        // 启动 Clock 监控
        let rpc_clone = rpc.clone();
        let clock_clone = monitor.clock.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(new_clock) = get_clock(&rpc_clone).await {
                    let mut clock_guard = clock_clone.lock().await;
                    *clock_guard = new_clock;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        // 启动 Miner 监控
        let rpc_clone = rpc.clone();
        let payer_clone = payer.clone();
        let miner_clone = monitor.miner.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(new_miner) = get_miner(&rpc_clone, payer_clone.pubkey()).await {
                    let mut miner_guard = miner_clone.lock().await;
                    *miner_guard = new_miner;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        // 启动 Round 监控
        let rpc_clone = rpc.clone();
        let board_clone = monitor.board.clone();
        let round_clone = monitor.round.clone();
        tokio::spawn(async move {
            loop {
                let round_id = {
                    let board = board_clone.lock().await;
                    board.round_id
                };

                if let Ok(new_round) = get_round(&rpc_clone, round_id).await {
                    let mut round_guard = round_clone.lock().await;
                    *round_guard = new_round;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        info!("✅ 实时监控系统已启动");
        Ok(())
    }

    /// 获取当前状态快照
    pub async fn get_snapshot(&self) -> MonitorSnapshot {
        MonitorSnapshot {
            board: self.board.lock().await.clone(),
            clock: self.clock.lock().await.clone(),
            miner: self.miner.lock().await.clone(),
            round: self.round.lock().await.clone(),
        }
    }
}

/// 监控状态快照
#[derive(Clone)]
pub struct MonitorSnapshot {
    pub board: Board,
    pub clock: Clock,
    pub miner: Miner,
    pub round: Round,
}

impl MonitorSnapshot {
    /// 计算剩余时间（秒）
    pub fn time_remaining(&self) -> f64 {
        if self.board.end_slot > self.clock.slot {
            (self.board.end_slot - self.clock.slot) as f64 * 0.4
        } else {
            0.0
        }
    }

    /// 计算剩余 slots
    pub fn slots_remaining(&self) -> u64 {
        self.board.end_slot.saturating_sub(self.clock.slot)
    }

    /// 检查是否应该部署（基于剩余 slots）
    pub fn should_deploy(&self, remaining_slots_threshold: u64) -> bool {
        self.slots_remaining() <= remaining_slots_threshold
    }

    /// 显示状态信息
    pub fn log_status(&self) {
        info!("┌─────────────────────────────────────────────────────┐");
        info!("│ 📊 挖矿状态                                         │");
        info!("├─────────────────────────────────────────────────────┤");
        info!("│ Round ID: {}                                      │", self.board.round_id);
        info!("│ 当前 Slot: {}                              │", self.clock.slot);
        info!("│ 结束 Slot: {}                              │", self.board.end_slot);
        info!("│ 剩余时间: {:.2}s ({} slots)                      │", self.time_remaining(), self.slots_remaining());
        info!("│ Miner Round: {}                                   │", self.miner.round_id);
        info!("│ Checkpoint ID: {}                                 │", self.miner.checkpoint_id);
        info!("└─────────────────────────────────────────────────────┘");
    }
}
