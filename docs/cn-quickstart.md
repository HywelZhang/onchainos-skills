# CN 快速上手（大陆网络环境）

> 适用: 中国大陆网络 + Clash 本地代理（默认 127.0.0.1:7897）。所有 OKX 域名（www.okx.ai / web3.okx.com / web3.okx.ac 等）被 DNS 污染解析到 169.254.0.2；github.com 网页常超时但 api/raw/codeload 通常可达。

## 1. 代理

每个新 shell 会话先导出（环境变量不跨会话持久）:

```bash
export HTTPS_PROXY=http://127.0.0.1:7897 HTTP_PROXY=http://127.0.0.1:7897 ALL_PROXY=http://127.0.0.1:7897
```

PowerShell 内联同样要设:

```powershell
$env:HTTPS_PROXY='http://127.0.0.1:7897'; $env:HTTP_PROXY='http://127.0.0.1:7897'
```

验证: `curl -s -o /dev/null -w "%{http_code}" -x http://127.0.0.1:7897 https://www.google.com`

## 2. 安装 / 升级 onchainos CLI

```powershell
# 代理需在 PowerShell 内部设置(见上)
irm https://raw.githubusercontent.com/okx/onchainos-skills/main/install.ps1 | iex
```

- 装到 `%USERPROFILE%\.local\bin\onchainos.exe`（已在 PATH 则直接 `onchainos`）
- Windows 下 install.ps1 可能打印非致命警告 "could not sync workflows (tar ... resolve failed)" — 可忽略
- raw.githubusercontent.com 偶发 HTTP 200 但 0 字节: 重试或改走 codeload

## 3. 同步 fork skills 到 Hermes（推荐，替代 hermes skills install）

`hermes skills install <url>` 只装 SKILL.md + references/，会丢 `_shared/` 子目录（跨 skill preflight 引用因此断链），且 URL 方式烧 GitHub API 配额。用仓库内脚本整目录同步:

```bash
bash scripts/sync-to-hermes.sh            # 自动定位 Hermes skills 目录, 自动备份, 校验文件数
# 或
powershell -ExecutionPolicy Bypass -File scripts/sync-to-hermes.ps1
```

备份在 skills 目录之外（skills-backup-<时间戳>），不会被 Hermes 当作 skill 扫描到。

## 4. 升级后必做

```bash
onchainos preflight --skill-version <SKILL.md frontmatter 里的版本>   # data.action=null 即通过
onchainos --version                                                    # CLI 版本
```

skill 文件版本与 CLI 版本需要配套（见 docs/design/03 开放问题）。上游 sync 后重跑事件审计:

```bash
python scripts/audit-events.py
```

## 5. 配额

免费市场数据有配额（基础 1M/月 + 30 天宽限），撞墙返回 `MARKET_API_*_OVER_QUOTA`。生产/常用: 到 OKX Developer Portal (https://web3.okx.com/onchain-os/dev-portal) 申请 OKX_API_KEY / OKX_SECRET_KEY / OKX_PASSPHRASE 写入 .env（切勿提交 git / 截图 / 聊天）。
