#!/usr/bin/env bash
# 在 Linux / WSL 上准备 okx-ai 的**写路径**（P0-11 v1.2 §12.5 决定：写路径放 Linux，绕开
# Windows 上 okx-a2a Hermes 网关插件不受支持的官方限制）。
#
# 用法:
#   bash scripts/setup-linux-writepath.sh                 # 安装/升级 CLI + okx-a2a 并自检
#   bash scripts/setup-linux-writepath.sh --version v4.6.3
#   bash scripts/setup-linux-writepath.sh --check-only    # 只自检，不下载
#
# 脚本只做"可自动化"的部分；`onchainos wallet login` 是社交登录（需浏览器）与
# `okx-a2a setup` 的平台绑定必须人工执行，脚本会在最后打印这两步。
set -euo pipefail

REPO="okx/onchainos-skills"
BIN_NAME="onchainos"
INSTALL_DIR="${HOME}/.local/bin"
VERSION=""
CHECK_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="${2:-}"; shift 2 ;;
    --check-only) CHECK_ONLY=1; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

log()  { printf '\033[36m[setup]\033[0m %s\n' "$*"; }
ok()   { printf '\033[32m[ ok ]\033[0m %s\n' "$*"; }
warn() { printf '\033[33m[warn]\033[0m %s\n' "$*"; }
die()  { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; exit 1; }

# ── 0. 平台与架构 ───────────────────────────────────────────────────────────
uname_s="$(uname -s)"
case "$uname_s" in
  Linux)  ;;
  Darwin) warn "macOS 也可用（脚本按 Linux 资产名选择；macOS 请把 target 换成 *-apple-darwin）" ;;
  *) die "本脚本用于 Linux/WSL；当前是 $uname_s" ;;
esac

arch="$(uname -m)"
case "$arch" in
  x86_64|amd64) target="x86_64-unknown-linux-musl" ;;
  aarch64|arm64) target="aarch64-unknown-linux-musl" ;;
  *) die "不支持的架构: $arch" ;;
esac
asset="${BIN_NAME}-${target}"
log "平台 $uname_s/$arch → 资产 $asset"

# ── 1. Node（okx-a2a 需要 >= 22.14）─────────────────────────────────────────
need_node=1
if command -v node >/dev/null 2>&1; then
  node_ver="$(node -v)"
  major="${node_ver#v}"; major="${major%%.*}"
  if [[ "$major" -ge 22 ]]; then ok "node $node_ver"; need_node=0
  else warn "node $node_ver 过旧（okx-a2a 需 >= 22.14）"; fi
fi
if [[ "$need_node" == 1 && "$CHECK_ONLY" == 0 ]]; then
  warn "请先安装 Node 22+：https://nodejs.org 或 (WSL) curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - && sudo apt-get install -y nodejs"
fi

# ── 2. onchainos CLI（GitHub release + checksum 校验）───────────────────────
download() {  # url out  (最多重试 3 次；本机 GitHub 访问是间歇性的)
  local url="$1" out="$2" i=1
  while [[ $i -le 3 ]]; do
    if curl -fsSL --retry 2 --connect-timeout 20 --max-time 300 "$url" -o "$out"; then return 0; fi
    warn "下载失败（第 $i 次）: $url"; i=$((i+1)); sleep 5
  done
  return 1
}

if [[ "$CHECK_ONLY" == 0 ]]; then
  mkdir -p "$INSTALL_DIR"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  if [[ -z "$VERSION" ]]; then
    log "查询最新 release…"
    VERSION="$(curl -fsSL --connect-timeout 20 "https://api.github.com/repos/${REPO}/releases/latest" \
      | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)"
    [[ -n "$VERSION" ]] || die "无法获取最新版本（网络？可显式传 --version v4.6.3）"
  fi
  base="https://github.com/${REPO}/releases/download/${VERSION}"
  log "下载 ${VERSION} / ${asset}"
  download "${base}/${asset}"            "${tmp}/${asset}"         || die "CLI 下载失败"
  download "${base}/checksums.txt"       "${tmp}/checksums.txt"    || die "checksums 下载失败"

  want="$(grep " ${asset}\$\\|${asset}\$" "${tmp}/checksums.txt" | awk '{print $1}' | head -1)"
  got="$(sha256sum "${tmp}/${asset}" | awk '{print $1}')"
  [[ -n "$want" ]]      || die "checksums.txt 中没有 ${asset} 的条目"
  [[ "$want" == "$got" ]] || die "校验和不一致: expected=$want actual=$got（拒绝安装）"
  ok "sha256 校验通过 $got"

  install -m 0755 "${tmp}/${asset}" "${INSTALL_DIR}/${BIN_NAME}"
  ok "已安装 → ${INSTALL_DIR}/${BIN_NAME}"
  case ":$PATH:" in *":${INSTALL_DIR}:"*) ;; *) warn "把 ${INSTALL_DIR} 加入 PATH: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;; esac

  # ── 3. okx-a2a（Node 包，Linux 上 setup 支持 bash → Hermes 插件可正常安装）──
  if [[ "$need_node" == 0 ]]; then
    log "安装/升级 @okxweb3/a2a-node"
    npm i -g @okxweb3/a2a-node@latest >/dev/null 2>&1 && ok "okx-a2a $(okx-a2a --version 2>/dev/null | head -1)" \
      || warn "npm 安装失败（稍后重试：npm i -g @okxweb3/a2a-node@latest）"
  fi
fi

# ── 4. 自检 ─────────────────────────────────────────────────────────────────
echo
log "自检"
command -v "$BIN_NAME" >/dev/null 2>&1 && ok "onchainos $("$BIN_NAME" --version 2>&1 | head -1)" || warn "onchainos 不在 PATH"
command -v okx-a2a  >/dev/null 2>&1 && ok "okx-a2a $(okx-a2a --version 2>&1 | head -1)"      || warn "okx-a2a 未安装"

echo
cat <<'NEXT'
──────────────────────────────────────────────────────────────────────────────
接下来必须**人工**完成这两步（脚本无法代做）：

1) 登录（社交登录，需浏览器；WSL 里不会自动弹窗，请把输出的 URL 复制到 Windows 浏览器打开）
     onchainos wallet login --help     # 依据实测版本选择 init/poll 形式
     # 建议同时固定凭据存储，避免依赖桌面 keyring 服务：
     export ONCHAINOS_FORCE_FILE_KEYRING=1

2) 让 A2A 运行时就绪（Linux 上支持 bash → Hermes 插件可装）
     okx-a2a setup hermes --json        # 或 setup codex / claude，按你希望的 provider
     okx-a2a doctor --fix --json        # 期望 ready:true

完成后在**同一个环境里**跑验证（gateway 与 CLI 必须同机同用户，因为写路径是委托 CLI）：
     export OKXAI_ONCHAINOS_CLI="$(command -v onchainos)"
     PY=$(command -v python3)
     $PY gateway/tests/probe_signing.py    # 签名链（HPKE/Ed25519）真机验证
     $PY gateway/tests/probe_writes.py     # 无签名写路径往返
     $PY gateway/tests/contract_golden.py  # 读路径契约（对照 4.5.2 录制的 golden）
最后才是花钱的那一步：task.create（0.1 USDT 预算，见 docs/design/12 的验收清单）
──────────────────────────────────────────────────────────────────────────────
NEXT
