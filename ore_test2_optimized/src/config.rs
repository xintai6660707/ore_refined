use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 完整的配置文件结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 通用配置（所有策略共用）
    pub common: CommonConfig,

    /// 策略配置
    pub strategy: StrategyConfig,

    /// 高级配置（可选）
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

/// 通用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonConfig {
    /// RPC 节点地址
    pub rpc: String,

    /// Keypair 文件路径
    pub keypair: String,

    /// 部署时机配置
    pub timing: TimingConfig,
}

/// 部署时机配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// 提前部署时间（秒）
    #[serde(default = "default_start_before_seconds")]
    pub start_before_seconds: f64,

    /// 剩余 slots 阈值
    #[serde(default = "default_remaining_slots")]
    pub remaining_slots: u64,
}

/// 策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StrategyConfig {
    /// 固定阈值策略
    FixedThreshold {
        #[serde(flatten)]
        params: FixedThresholdParams,
    },

    /// 动态优化策略
    DynamicOptimized {
        #[serde(flatten)]
        params: DynamicOptimizedParams,
    },
}

/// 固定阈值策略参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedThresholdParams {
    /// 固定阈值（SOL）- 低于此值的格子才会被选择
    pub threshold_sol: f64,

    /// 每个格子部署的 SOL 数量
    #[serde(default = "default_amount_sol")]
    pub amount_sol: f64,

    /// 最少需要满足条件的格子数量
    #[serde(default = "default_min_squares")]
    pub min_squares: usize,

    /// 实际选择部署的格子数量
    #[serde(default = "default_pick_squares")]
    pub pick_squares: usize,
}

/// 动态优化策略参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicOptimizedParams {
    /// 每个格子部署的 SOL 数量
    #[serde(default = "default_amount_sol")]
    pub amount_sol: f64,

    /// 最少需要满足条件的格子数量
    #[serde(default = "default_min_squares")]
    pub min_squares: usize,

    /// 实际选择部署的格子数量
    #[serde(default = "default_pick_squares")]
    pub pick_squares: usize,

    /// 动态阈值计算系数（默认 0.036）
    #[serde(default = "default_dynamic_coefficient")]
    pub dynamic_coefficient: f64,

    /// 动态阈值偏移量（默认 -0.005）
    #[serde(default = "default_dynamic_offset")]
    pub dynamic_offset: f64,
}

/// 高级配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// 计算单元价格（优先费，lamports）
    #[serde(default = "default_compute_unit_price")]
    pub compute_unit_price: u64,

    /// 计算单元限制
    #[serde(default = "default_compute_unit_limit")]
    pub compute_unit_limit: u64,

    /// Jito 小费金额（lamports）
    #[serde(default = "default_jito_tip")]
    pub jito_tip: u64,

    /// 是否启用 Jito Bundle 提交
    #[serde(default = "default_enable_jito")]
    pub enable_jito: bool,

    /// 最大重试次数
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// 日志级别 (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

// 默认值函数
fn default_start_before_seconds() -> f64 { 40.0 }
fn default_remaining_slots() -> u64 { 15 }
fn default_amount_sol() -> f64 { 0.01 }
fn default_min_squares() -> usize { 12 }
fn default_pick_squares() -> usize { 5 }
fn default_dynamic_coefficient() -> f64 { 0.036 }
fn default_dynamic_offset() -> f64 { -0.005 }
fn default_compute_unit_price() -> u64 { 20_000 }
fn default_compute_unit_limit() -> u64 { 400_000 }
fn default_jito_tip() -> u64 { 5_000 }
fn default_enable_jito() -> bool { true }
fn default_max_retries() -> u32 { 4 }
fn default_log_level() -> String { "info".to_string() }

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            compute_unit_price: default_compute_unit_price(),
            compute_unit_limit: default_compute_unit_limit(),
            jito_tip: default_jito_tip(),
            enable_jito: default_enable_jito(),
            max_retries: default_max_retries(),
            log_level: default_log_level(),
        }
    }
}

impl Config {
    /// 从 JSON 文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow::anyhow!("无法读取配置文件: {}", e))?;

        let config: Config = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("配置文件格式错误: {}", e))?;

        // 验证配置
        config.validate()?;

        Ok(config)
    }

    /// 验证配置的有效性
    fn validate(&self) -> Result<(), anyhow::Error> {
        // 验证 RPC 地址
        if self.common.rpc.is_empty() {
            anyhow::bail!("RPC 地址不能为空");
        }

        // 验证 keypair 路径
        if self.common.keypair.is_empty() {
            anyhow::bail!("Keypair 路径不能为空");
        }

        // 验证时机参数
        if self.common.timing.start_before_seconds < 0.0 {
            anyhow::bail!("start_before_seconds 必须为正数");
        }

        // 验证策略特定参数
        match &self.strategy {
            StrategyConfig::FixedThreshold { params } => {
                if params.threshold_sol <= 0.0 {
                    anyhow::bail!("threshold_sol 必须大于 0");
                }
                if params.amount_sol <= 0.0 {
                    anyhow::bail!("amount_sol 必须大于 0");
                }
                if params.min_squares == 0 || params.min_squares > 25 {
                    anyhow::bail!("min_squares 必须在 1-25 之间");
                }
                if params.pick_squares == 0 || params.pick_squares > params.min_squares {
                    anyhow::bail!("pick_squares 必须在 1-{} 之间", params.min_squares);
                }
            }
            StrategyConfig::DynamicOptimized { params } => {
                if params.amount_sol <= 0.0 {
                    anyhow::bail!("amount_sol 必须大于 0");
                }
                if params.min_squares == 0 || params.min_squares > 25 {
                    anyhow::bail!("min_squares 必须在 1-25 之间");
                }
                if params.pick_squares == 0 || params.pick_squares > params.min_squares {
                    anyhow::bail!("pick_squares 必须在 1-{} 之间", params.min_squares);
                }
                if params.dynamic_coefficient <= 0.0 {
                    anyhow::bail!("dynamic_coefficient 必须大于 0");
                }
            }
        }

        Ok(())
    }

    /// 保存配置到 JSON 文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), anyhow::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("序列化配置失败: {}", e))?;

        fs::write(path.as_ref(), json)
            .map_err(|e| anyhow::anyhow!("写入配置文件失败: {}", e))?;

        Ok(())
    }

    /// 打印配置摘要
    pub fn print_summary(&self) {
        println!("┌─────────────────────────────────────────────────────┐");
        println!("│ 📋 配置摘要                                         │");
        println!("├─────────────────────────────────────────────────────┤");
        println!("│ RPC: {}  │", truncate_string(&self.common.rpc, 40));
        println!("│ Keypair: {}                │", truncate_string(&self.common.keypair, 35));
        println!("├─────────────────────────────────────────────────────┤");

        match &self.strategy {
            StrategyConfig::FixedThreshold { params } => {
                println!("│ 策略: 固定阈值算法                                 │");
                println!("│   - 阈值: {:.6} SOL                             │", params.threshold_sol);
                println!("│   - 部署量: {:.6} SOL                           │", params.amount_sol);
                println!("│   - 最少格子: {}                                 │", params.min_squares);
                println!("│   - 选择格子: {}                                 │", params.pick_squares);
            }
            StrategyConfig::DynamicOptimized { params } => {
                println!("│ 策略: 动态优化算法                                 │");
                println!("│   - 部署量: {:.6} SOL                           │", params.amount_sol);
                println!("│   - 最少格子: {}                                 │", params.min_squares);
                println!("│   - 选择格子: {}                                 │", params.pick_squares);
                println!("│   - 动态系数: {:.4}                             │", params.dynamic_coefficient);
                println!("│   - 动态偏移: {:.4}                             │", params.dynamic_offset);
            }
        }

        println!("├─────────────────────────────────────────────────────┤");
        println!("│ 部署时机:                                           │");
        println!("│   - 提前时间: {:.1}s                               │", self.common.timing.start_before_seconds);
        println!("│   - 剩余 Slots: {}                                 │", self.common.timing.remaining_slots);
        println!("├─────────────────────────────────────────────────────┤");
        println!("│ 高级设置:                                           │");
        println!("│   - Gas 价格: {} lamports                          │", self.advanced.compute_unit_price);
        println!("│   - Jito 小费: {} lamports                         │", self.advanced.jito_tip);
        println!("│   - 启用 Jito: {}                                  │", if self.advanced.enable_jito { "是" } else { "否" });
        println!("│   - 最大重试: {} 次                                │", self.advanced.max_retries);
        println!("└─────────────────────────────────────────────────────┘");
    }
}

/// 截断字符串并添加省略号
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len-3])
    }
}
