#!/bin/bash

# 遇到错误立即停止执行
set -e

echo "=== Installing Rust tools ==="

# 安装 Rust 常用工具
# 注意：cargo install 比较耗时，请耐心等待
cargo install cargo-generate
cargo install --locked cargo-deny

# 初始化并检查 cargo-deny (通常在具体项目目录下运行，这里按原脚本保留逻辑)
cargo deny init || echo "cargo-deny already initialized"
cargo deny fetch
cargo deny check || echo "cargo-deny check failed (normal if no config yet)"

cargo install typos-cli
cargo install git-cliff
# 初始化 git-cliff 配置
git cliff --init
cargo install --locked cargo-nextest

echo "[OK] Rust tools installed successfully!"
echo ""

# ===============================================
# uv 设置 (WSL 环境下通常安装在 ~/.local/bin)
# ===============================================

echo "=== Installing uv ==="

# 检查 uv 是否已安装
if ! command -v uv &> /dev/null; then
    echo "Installing uv..."
    # 使用 Linux 版安装脚本
    curl -LsSf https://astral.sh/uv/install.sh | sh
    # 立即将 uv 路径加入当前 session
    source $HOME/.cargo/env
    # 兼容性设置：确保 PATH 包含 ~/.local/bin
    export PATH="$HOME/.local/bin:$PATH"
fi

# 设置 pip 镜像（加速）
export UV_PIP_INDEX_URL="https://mirrors.aliyun.com/pypi/simple/"

echo "=== Creating Virtual Environment ==="
# 创建带 pip 的虚拟环境 (Python 3.12)
# 注意：如果系统没装 python3.12，uv 会尝试自动下载它
uv venv --python 3.12 --seed

# 激活虚拟环境 (Linux 下使用 source)
source .venv/bin/activate

# 安装 pre-commit
uv pip install pre-commit

# 安装 Git 钩子
if [ -d ".git" ]; then
    pre-commit install
    echo "[OK] Git hooks installed."
else
    echo "[Warn] Not a git repository, skipping pre-commit install."
fi

echo ""
echo "[OK] uv, virtual environment, and pre-commit are ready!"
echo "[OK] All tools installed successfully!"
