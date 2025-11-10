# 🚀 快速开始指南

## 📍 项目位置

```
/home/user/ore_refined/ore_test2_optimized/
```

---

## ⚡ 一键开始（3步）

### 1️⃣ 配置 ore-api 依赖

编辑 `Cargo.toml` 文件，取消注释以下其中一行：

```toml
# 方式1：本地路径（如果你有 ore 项目在本地）
ore-api = { path = "../ore/api" }

# 方式2：Git 仓库
ore-api = { git = "https://github.com/regolith-labs/ore" }
```

### 2️⃣ 构建项目

```bash
cd /home/user/ore_refined/ore_test2_optimized
cargo build --release
```

### 3️⃣ 运行挖矿

```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  auto-optimized \
  --amount-sol 0.01 \
  --min-squares 12 \
  --pick-squares 5
```

---

## 📖 常用命令

### 查看余额
```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  balance
```

### 查看状态
```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  status
```

### 领取奖励
```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  claim
```

---

## 📂 项目文件说明

| 文件 | 说明 |
|------|------|
| `src/main.rs` | 主程序入口 |
| `src/monitor.rs` | 实时监控系统（核心） |
| `src/jito.rs` | Jito Bundle 集成 |
| `src/price.rs` | 价格监控 |
| `src/utils.rs` | 工具函数 |
| `Cargo.toml` | 项目配置 |
| `README.md` | 完整文档 |
| `OPTIMIZATION_GUIDE.md` | 详细优化说明 |

---

## ⚙️ 推荐参数

### 阈值算法（适合新手）
```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  auto-threshold \
  --amount-sol 0.01 \
  --threshold-sol 0.01 \
  --min-squares 12 \
  --pick-squares 5 \
  --remaining-slots 15
```

### 最优化算法（自动计算阈值）
```bash
./target/release/ore-test2-optimized \
  --rpc YOUR_RPC_URL \
  --keypair YOUR_KEYPAIR_PATH \
  auto-optimized \
  --amount-sol 0.01 \
  --min-squares 12 \
  --pick-squares 5 \
  --remaining-slots 15
```

---

## 🔧 故障排除

### 构建失败？
1. 检查是否配置了 `ore-api` 依赖
2. 确认 Rust 版本 >= 1.70
3. 尝试：`cargo clean && cargo build --release`

### 运行失败？
1. 检查 RPC URL 是否可访问
2. 确认 keypair 文件路径正确
3. 查看日志输出的错误信息

---

## 📚 更多文档

- [README.md](./README.md) - 完整使用文档
- [OPTIMIZATION_GUIDE.md](./OPTIMIZATION_GUIDE.md) - 详细优化说明

---

**开始挖矿吧！** 🎉
