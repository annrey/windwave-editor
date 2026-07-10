# Multica-Bridge 开发部署指南

## 目录
- [开发环境搭建](#开发环境搭建)
- [项目结构](#项目结构)
- [编译与测试](#编译与测试)
- [部署流程](#部署流程)
- [配置管理](#配置管理)
- [调试与日志](#调试与日志)
- [性能优化](#性能优化)
- [CI/CD 集成](#cicd-集成)

## 开发环境搭建

### 1. 系统要求

| 组件 | 最低要求 | 推荐配置 |
|------|----------|----------|
| 操作系统 | macOS 12+ / Ubuntu 20.04+ / Windows 10+ | macOS 13+ / Ubuntu 22.04+ |
| CPU | 4 核 | 8 核+ |
| 内存 | 8GB | 16GB+ |
| 磁盘 | 10GB 可用空间 | SSD, 20GB+ 可用空间 |
| 网络 | 宽带互联网连接 | 稳定网络连接 |

### 2. 安装依赖

#### macOS

```bash
# 安装 Homebrew (如未安装)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 安装 Rust
brew install rustup
rustup-init

# 安装其他工具
brew install git pkg-config openssl

# 配置 Rust 工具链
rustup default stable
rustup update
```

#### Ubuntu/Debian

```bash
# 安装系统依赖
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl

# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 配置 Rust 工具链
rustup default stable
rustup update
```

#### Windows

1. 安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. 安装 [Rust](https://rustup.rs/)
3. 安装 [Git for Windows](https://gitforwindows.org/)

### 3. 克隆项目

```bash
# 克隆仓库
git clone <repository-url>
cd 风浪

# 初始化子模块（如有）
git submodule update --init --recursive
```

### 4. 验证安装

```bash
# 检查 Rust 版本
rustc --version
cargo --version

# 编译项目
cargo build

# 运行测试
cargo test -p multica-bridge
```

## 项目结构

```
风浪/
├── crates/
│   ├── multica-bridge/          # 核心桥接模块
│   │   ├── src/
│   │   │   ├── lib.rs           # 库入口
│   │   │   ├── types.rs         # 类型定义
│   │   │   ├── scene_memory.rs  # 场景记忆
│   │   │   ├── memory_injector.rs # 记忆注入器
│   │   │   ├── ws_client.rs     # WebSocket 客户端
│   │   │   ├── task_sync_module.rs # 任务同步
│   │   │   ├── message_handler.rs # 消息处理器
│   │   │   ├── multica_daemon.rs # 守护进程
│   │   │   └── ...              # 其他模块
│   │   ├── tests/               # 集成测试
│   │   │   └── e2e_integration.rs # 端到端测试
│   │   └── Cargo.toml           # 模块配置
│   ├── agent-core/              # Agent 核心
│   ├── agent-ui/                # Agent UI
│   └── ...                      # 其他模块
├── docs/                        # 文档
│   ├── multica-bridge-api-reference.md
│   ├── multica-bridge-architecture.md
│   ├── multica-bridge-user-guide.md
│   └── multica-bridge-deployment-guide.md
├── Cargo.toml                   # 工作区配置
└── README.md                    # 项目说明
```

## 编译与测试

### 1. 基本编译

```bash
# 编译整个项目
cargo build

# 编译特定模块
cargo build -p multica-bridge

# 发布模式编译（优化）
cargo build --release
```

### 2. 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test -p multica-bridge

# 运行单个测试
cargo test -p multica-bridge test_e2e_full_integration_scenario

# 运行测试并显示输出
cargo test -p multica-bridge -- --nocapture
```

### 3. 代码质量检查

```bash
# 代码格式化
cargo fmt

# 检查格式
cargo fmt --check

# 代码检查
cargo clippy

# 检查代码（更严格）
cargo clippy -- -D warnings
```

### 4. 性能分析

```bash
# 启用性能分析标志编译
cargo build --release --features profiling

# 使用 cargo-flamegraph 生成火焰图
cargo install cargo-flamegraph
cargo flamegraph --bin your-binary
```

## 部署流程

### 1. 本地开发

```bash
# 1. 克隆代码
git clone <repository-url>
cd 风浪

# 2. 安装依赖
cargo build

# 3. 运行测试
cargo test -p multica-bridge

# 4. 启动开发服务器（如适用）
cargo run --bin your-binary
```

### 2. 生产部署

#### 步骤 1: 准备构建环境

```bash
# 更新系统
sudo apt update && sudo apt upgrade -y

# 安装依赖
sudo apt install -y build-essential pkg-config libssl-dev
```

#### 步骤 2: 编译发布版本

```bash
# 克隆或拉取最新代码
git pull origin main

# 编译发布版本
cargo build --release

# 二进制文件位置
ls target/release/
```

#### 步骤 3: 部署到服务器

```bash
# 复制二进制文件
scp target/release/your-binary user@server:/opt/multica-bridge/

# 复制配置文件
scp config/production.toml user@server:/opt/multica-bridge/

# 设置权限
ssh user@server "chmod +x /opt/multica-bridge/your-binary"
```

#### 步骤 4: 配置服务

创建 systemd 服务文件 `/etc/systemd/system/multica-bridge.service`:

```ini
[Unit]
Description=Multica Bridge Service
After=network.target

[Service]
Type=simple
User=multica
WorkingDirectory=/opt/multica-bridge
ExecStart=/opt/multica-bridge/your-binary --config /opt/multica-bridge/production.toml
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启动服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable multica-bridge
sudo systemctl start multica-bridge
sudo systemctl status multica-bridge
```

### 3. Docker 部署

#### Dockerfile

```dockerfile
# 构建阶段
FROM rust:1.75-slim AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/your-binary .
COPY config/production.toml .

EXPOSE 8080

CMD ["./your-binary", "--config", "production.toml"]
```

#### 构建和运行

```bash
# 构建镜像
docker build -t multica-bridge:latest .

# 运行容器
docker run -d \
    --name multica-bridge \
    -p 8080:8080 \
    -v /path/to/config:/app/config \
    multica-bridge:latest

# 查看日志
docker logs -f multica-bridge
```

## 配置管理

### 1. 配置文件

创建 `config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 100

[multica]
server_url = "ws://localhost:3000"
auth_token = "your-token-here"
reconnect_interval_secs = 5
max_reconnect_attempts = 10

[memory]
max_scenes_cached = 100
enable_working_memory = true
enable_episodic_memory = true
enable_semantic_memory = true
enable_procedural_memory = true

[sync]
direction = "Bidirectional"
conflict_resolution = "LastWriteWins"
auto_sync_interval_secs = 30

[logging]
level = "info"
file = "logs/multica-bridge.log"
```

### 2. 环境变量

```bash
# 服务器配置
export MULTICA_BRIDGE_HOST=0.0.0.0
export MULTICA_BRIDGE_PORT=8080

# Multica 配置
export MULTICA_SERVER_URL=ws://localhost:3000
export MULTICA_AUTH_TOKEN=your-token

# 日志配置
export RUST_LOG=info
```

### 3. 配置优先级

1. 命令行参数（最高优先级）
2. 环境变量
3. 配置文件
4. 默认值（最低优先级）

## 调试与日志

### 1. 启用调试日志

```bash
# 通过环境变量
export RUST_LOG=debug

# 通过命令行
cargo run -- --log-level debug
```

### 2. 日志格式

```
2024-01-01T12:00:00Z INFO  multica_bridge::ws_client: Connected to server
2024-01-01T12:00:01Z DEBUG multica_bridge::task_sync: Syncing tasks...
2024-01-01T12:00:02Z ERROR multica_bridge::memory_injector: Failed to inject memory: ...
```

### 3. 常用调试技巧

#### 打印变量

```rust
use tracing::{debug, info, warn, error};

debug!("Variable value: {:?}", my_variable);
info!("Processing task: {}", task_id);
warn!("Connection unstable");
error!("Failed to sync: {}", error);
```

#### 性能计时

```rust
use std::time::Instant;

let start = Instant::now();
// ... 执行操作 ...
let duration = start.elapsed();
info!("Operation took {:?}", duration);
```

#### 内存分析

```bash
# 使用 valgrind
valgrind --leak-check=full ./target/release/your-binary

# 使用 cargo-memcpy
cargo install cargo-memcpy
cargo memcpy --bin your-binary
```

## 性能优化

### 1. 编译优化

在 `Cargo.toml` 中配置：

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 2. 运行时优化

- **减少内存分配**: 使用 `&str` 而非 `String`，使用 `Cow` 避免不必要的克隆
- **异步操作**: 使用 `tokio` 处理 I/O 密集型任务
- **缓存**: 缓存频繁访问的数据（如场景快照）
- **批处理**: 批量处理消息和同步操作

### 3. 并发优化

```rust
// 使用 Arc<Mutex<T>> 实现线程安全
let shared_data = Arc::new(Mutex::new(MyData::new()));

// 克隆 Arc 用于多个线程
let data_clone = shared_data.clone();
tokio::spawn(async move {
    // 使用 data_clone
});
```

## CI/CD 集成

### 1. GitHub Actions

创建 `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3

    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true

    - name: Cache dependencies
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

    - name: Build
      run: cargo build --verbose

    - name: Run tests
      run: cargo test --verbose

    - name: Run clippy
      run: cargo clippy -- -D warnings

    - name: Check formatting
      run: cargo fmt --check
```

### 2. 自动化部署

```yaml
deploy:
  needs: build
  runs-on: ubuntu-latest
  if: github.ref == 'refs/heads/main'

  steps:
  - uses: actions/checkout@v3

  - name: Deploy to server
    run: |
      # 部署脚本
      scp target/release/your-binary user@server:/opt/multica-bridge/
      ssh user@server "systemctl restart multica-bridge"
```

## 常见问题

### Q: 编译速度慢怎么办？

A: 使用以下优化：
```bash
# 使用 mold 链接器
sudo apt install mold
export RUSTFLAGS="-C link-arg=-fuse-ld=mold"

# 增加并行编译
cargo build -j$(nproc)
```

### Q: 如何处理依赖冲突？

A: 
```bash
# 更新依赖
cargo update

# 查看依赖树
cargo tree

# 清理缓存
cargo clean
cargo build
```

### Q: 如何监控生产环境？

A: 
- 使用 Prometheus + Grafana 监控指标
- 使用 ELK Stack 收集和分析日志
- 配置告警规则（如错误率、响应时间）

### Q: 如何回滚部署？

A: 
```bash
# 停止当前版本
sudo systemctl stop multica-bridge

# 恢复旧版本
cp /opt/multica-bridge/your-binary.backup /opt/multica-bridge/your-binary

# 重启服务
sudo systemctl start multica-bridge
```
