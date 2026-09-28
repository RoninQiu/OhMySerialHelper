#!/usr/bin/env bash
# scripts/release.sh — OhMySerial 版本发布脚本
#
# 用法：
#   ./scripts/release.sh <new_version> [--dry-run] [--skip-tests] [--build] [--notes <file>]
#
# 示例：
#   ./scripts/release.sh 1.0.2
#   ./scripts/release.sh 1.0.2 --dry-run        # 只演练不真发
#   ./scripts/release.sh 1.0.2 --skip-tests     # 跳测试（已跑过的情况）
#   ./scripts/release.sh 1.0.2 --build          # 同时跑 npm run tauri build 并上传 NSIS installer
#   ./scripts/release.sh 1.0.2 --notes docs/releases/1.0.2.md
#
# 流程：
#   1. 预检（git 干净 / 在 main / gh 已登录）
#   2. 同步三处 version 字段（Cargo.toml / tauri.conf.json / package.json）
#   2.5. 同步 README.md 的 shields.io badge + installer 文件名（机械的 2 处）
#   3. cargo check 重算 Cargo.lock
#   4. 跑测试（cargo test --lib + tsc + vitest），--skip-tests 跳过
#   5. commit + push
#   5.5. [可选 --build] npm run tauri build 产出 NSIS installer（5-15 分钟）
#   6. git tag + push tag
#   7. 写 / 校验 release notes（docs/releases/<version>.md）
#   8. gh release create + 上传 installer 到 assets（--dry-run 不真发）
#
# 前置：
#   - gh CLI 已登录（gh auth status 通过）
#   - 当前分支为 main
#   - 工作区干净（无未提交改动）
#   - 所有测试应在本地已跑过
set -euo pipefail

# ---- 颜色 ----
if [ -t 1 ]; then
  RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; BLUE=$'\033[34m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
else
  RED=""; GREEN=""; YELLOW=""; BLUE=""; BOLD=""; RESET=""
fi

info()  { printf "${BLUE}==>${RESET} %s\n" "$*"; }
ok()    { printf "${GREEN}✓${RESET} %s\n" "$*"; }
warn()  { printf "${YELLOW}!${RESET} %s\n" "$*"; }
err()   { printf "${RED}✗${RESET} %s\n" "$*" >&2; }

# ---- 解析参数 ----
DRY_RUN="false"
SKIP_TESTS="false"
DO_BUILD="false"
NOTES_FILE=""
NEW_VERSION=""

# 先扫描 --help / -h（不带版本号也能用）
for arg in "$@"; do
  if [ "$arg" = "-h" ] || [ "$arg" = "--help" ]; then
    sed -n '2,21p' "$0"
    exit 0
  fi
done

if [ $# -lt 1 ]; then
  err "缺少版本号"
  echo "用法: $0 <new_version> [--dry-run] [--skip-tests] [--notes <file>]"
  echo "示例: $0 1.0.2"
  exit 1
fi

NEW_VERSION="$1"; shift

while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)     DRY_RUN="true" ;;
    --skip-tests)  SKIP_TESTS="true" ;;
    --build)       DO_BUILD="true" ;;
    --notes)       NOTES_FILE="${2:-}"; shift ;;
    -h|--help)     sed -n '2,24p' "$0"; exit 0 ;;
    *)             err "未知参数: $1"; exit 1 ;;
  esac
  shift
done

# 校验版本号格式（X.Y.Z，可选 -rc1 / -beta1 后缀）
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  err "版本号格式不合法: $NEW_VERSION（应为 X.Y.Z 或 X.Y.Z-rc1）"
  exit 1
fi

# 校验 v 前缀（gh tag 通常带 v，可选）
TAG_NAME="v$NEW_VERSION"

# ---- 1. 预检 ----
info "预检"
if [ -n "$(git status --porcelain)" ]; then
  err "工作区有未提交改动："
  git status --short
  exit 1
fi
ok "工作区干净"

CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
  err "当前分支是 $CURRENT_BRANCH，应在 main 上发布"
  exit 1
fi
ok "在 main 分支"

# 校验本地 main 与远端一致（避免推 tag 后才发现有遗漏 commit）
LOCAL_SHA=$(git rev-parse HEAD)
REMOTE_SHA=$(git rev-parse origin/main 2>/dev/null || echo "")
if [ "$LOCAL_SHA" != "$REMOTE_SHA" ]; then
  err "本地 main ($LOCAL_SHA) 与远端 origin/main ($REMOTE_SHA) 不一致"
  err "请先 git push origin main 或 git pull --rebase"
  exit 1
fi
ok "本地与远端同步"

if [ -n "${GH_TOKEN:-}" ]; then
  ok "通过 GH_TOKEN 环境变量认证（gh auth login 旁路，PAT 需 repo scope）"
elif gh auth status >/dev/null 2>&1; then
  ok "gh CLI 已登录"
else
  err "gh CLI 未登录且 GH_TOKEN 未设置，请先 gh auth login 或 export GH_TOKEN=..."
  exit 1
fi

# 校验 tag 尚未存在
if git rev-parse "$TAG_NAME" >/dev/null 2>&1; then
  err "tag $TAG_NAME 已存在，请改用新版本号或先删除旧 tag"
  exit 1
fi
ok "tag $TAG_NAME 未被占用"

# ---- 取旧版本号（用于 sed 替换）----
OLD_VERSION=$(grep -E '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
info "版本: $OLD_VERSION → $NEW_VERSION"
if [ "$OLD_VERSION" = "$NEW_VERSION" ]; then
  err "新版本号与旧版本号相同"
  exit 1
fi

if [ "$DRY_RUN" = "true" ]; then
  warn "DRY-RUN 模式：所有写操作仅 echo，不真发"
fi

# ---- 2. 同步三处 version 字段 ----
info "[2/8] 同步 version 字段"
for f in src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json; do
  if ! grep -q "$OLD_VERSION" "$f"; then
    err "$f 里找不到旧版本号 $OLD_VERSION，请检查"
    exit 1
  fi
  if [ "$DRY_RUN" = "true" ]; then
    echo "  [dry-run] sed $f: $OLD_VERSION → $NEW_VERSION"
  else
    # macOS 与 Linux sed 行为不同：-i.bak 兼容两者，跑完删 .bak
    sed -i.bak "s/$OLD_VERSION/$NEW_VERSION/g" "$f"
    rm -f "$f.bak"
    ok "更新 $f"
  fi
done

# ---- 2.5. 同步 README.md 的机械 version 引用 ----
# 背景：v1.0.2 / v1.1.0 / v1.1.1 连续 3 次 release 都忘了改 README，
#       shields.io badge URL 和 installer 文件名仍然是旧版本号。
# 修复：纳入 release 脚本自动同步（仅机械的 2 处）：
#   1. shields.io badge：img.shields.io/badge/version-X.Y.Z-blue.svg
#   2. installer 文件名：OhMySerial_X.Y.Z_x64-setup.exe
#
# 不自动同步的（语义化、需要人写）：
#   - "💡 v1.X.Y 完整功能" 描述段
#   - "📥 OhMySerial_X.Y.Z_x64-setup.exe" 描述段
#   - 路线图列表（要加新行 + 写新版本描述）
info "[2.5/8] 同步 README.md 机械 version"
README_FILE="README.md"
# 1. shields.io badge：version-OLD → version-NEW
BADGE_OLD="version-$OLD_VERSION-blue.svg"
BADGE_NEW="version-$NEW_VERSION-blue.svg"
if [ "$DRY_RUN" = "true" ]; then
  if grep -q "$BADGE_OLD" "$README_FILE"; then
    echo "  [dry-run] sed $README_FILE: $BADGE_OLD → $BADGE_NEW"
  else
    warn "$README_FILE 里找不到 $BADGE_OLD（可能已更新或格式变了）"
  fi
else
  if grep -q "$BADGE_OLD" "$README_FILE"; then
    sed -i.bak "s/$BADGE_OLD/$BADGE_NEW/g" "$README_FILE"
    rm -f "$README_FILE.bak"
    ok "更新 $README_FILE shields.io badge"
  else
    warn "$README_FILE 里找不到 $BADGE_OLD（跳过，可能已更新或格式变了）"
  fi
fi
# 2. installer 文件名（在代码块内）：OhMySerial_OLD_x64-setup.exe → OhMySerial_NEW_x64-setup.exe
INSTALLER_OLD="OhMySerial_${OLD_VERSION}_x64-setup.exe"
INSTALLER_NEW="OhMySerial_${NEW_VERSION}_x64-setup.exe"
if [ "$DRY_RUN" = "true" ]; then
  if grep -q "$INSTALLER_OLD" "$README_FILE"; then
    echo "  [dry-run] sed $README_FILE: $INSTALLER_OLD → $INSTALLER_NEW"
  else
    warn "$README_FILE 里找不到 $INSTALLER_OLD（可能已更新或文件名格式变了）"
  fi
else
  if grep -q "$INSTALLER_OLD" "$README_FILE"; then
    sed -i.bak "s/$INSTALLER_OLD/$INSTALLER_NEW/g" "$README_FILE"
    rm -f "$README_FILE.bak"
    ok "更新 $README_FILE installer 文件名"
  else
    warn "$README_FILE 里找不到 $INSTALLER_OLD（跳过，可能已更新或文件名格式变了）"
  fi
fi

# ---- 3. 重算 Cargo.lock ----
info "[3/8] 重算 Cargo.lock"
if [ "$DRY_RUN" = "true" ]; then
  echo "  [dry-run] cd src-tauri && cargo check --quiet"
else
  (cd src-tauri && cargo check --quiet)
  ok "Cargo.lock 已更新"
fi

# ---- 4. 跑测试 ----
info "[4/8] 跑测试"
if [ "$SKIP_TESTS" = "true" ]; then
  warn "已跳过（--skip-tests）"
else
  if [ "$DRY_RUN" = "true" ]; then
    echo "  [dry-run] cargo test --lib && tsc && vitest"
  else
    (cd src-tauri && cargo test --lib 2>&1 | tail -3)
    npx tsc --noEmit
    npm test -- --run --reporter=basic
    ok "所有测试通过"
  fi
fi

# ---- 5. commit + push ----
info "[5/8] commit + push"
if [ "$DRY_RUN" = "true" ]; then
  echo "  [dry-run] git add -A && git commit -m 'chore(release): bump version to $NEW_VERSION'"
  echo "  [dry-run] git push origin main"
else
  git add -A
  git commit -m "chore(release): bump version to $NEW_VERSION"
  git push origin main
  ok "已推送 main"
fi

# ---- 5.5. (可选) 构建 NSIS installer ----
INSTALLER_PATH=""
if [ "$DO_BUILD" = "true" ]; then
  info "[5.5] 构建 NSIS installer（首次 5-15 分钟，增量更快）"
  if [ "$DRY_RUN" = "true" ]; then
    echo "  [dry-run] npm run tauri build"
    INSTALLER_PATH="target/release/bundle/nsis/OhMySerial_${NEW_VERSION}_x64-setup.exe"
  else
    npm run tauri build 2>&1 | tail -20
    # Tauri 2.x 在 Windows 下的 NSIS 产物路径
    INSTALLER_PATH="target/release/bundle/nsis/OhMySerial_${NEW_VERSION}_x64-setup.exe"
    if [ ! -f "$INSTALLER_PATH" ]; then
      err "找不到 installer: $INSTALLER_PATH"
      err "检查 target/release/bundle/nsis/ 目录实际产物名"
      ls -la "target/release/bundle/nsis/" 2>/dev/null
      exit 1
    fi
    INSTALLER_SIZE=$(du -h "$INSTALLER_PATH" | cut -f1)
    ok "installer 已生成: $INSTALLER_PATH ($INSTALLER_SIZE)"
  fi
else
  info "[5.5] 跳过 build（未传 --build）"
fi

# ---- 6. 打 tag + 推送 ----
info "[6/8] 打 tag $TAG_NAME"
if [ "$DRY_RUN" = "true" ]; then
  echo "  [dry-run] git tag -a $TAG_NAME && git push origin $TAG_NAME"
else
  git tag -a "$TAG_NAME" -m "$NEW_VERSION"
  git push origin "$TAG_NAME"
  ok "tag $TAG_NAME 已推送"
fi

# ---- 7. release notes ----
info "[7/8] 准备 release notes"
DEFAULT_NOTES="docs/releases/${NEW_VERSION}.md"
if [ -n "$NOTES_FILE" ]; then
  : # 用户指定了，用指定的
elif [ -f "$DEFAULT_NOTES" ]; then
  NOTES_FILE="$DEFAULT_NOTES"
  ok "找到已有 notes: $NOTES_FILE"
else
  if [ "$DRY_RUN" = "true" ]; then
    NOTES_FILE="$DEFAULT_NOTES"
    echo "  [dry-run] 会生成 $DEFAULT_NOTES 模板"
  else
    mkdir -p docs/releases
    cat > "$DEFAULT_NOTES" <<EOF
# $NEW_VERSION

## 变更

<!-- 列出本次发布的关键改动；可参考 git log v$OLD_VERSION..HEAD -->

## 验证

- Rust 单测：$(cd src-tauri && cargo test --lib 2>&1 | grep -oE '[0-9]+ passed; [0-9]+ failed' | head -1 || echo "N/A")
- 前端单测：$(npm test -- --run 2>&1 | grep -oE 'Tests *[0-9]+ passed' | head -1 || echo "N/A")
- TypeScript 编译：0 错误
EOF
    NOTES_FILE="$DEFAULT_NOTES"
    warn "已生成 notes 模板，请编辑后重跑脚本（脚本会优先使用已存在的 notes）"
    err "请填写 $NOTES_FILE 后重跑（先 git reset --hard HEAD~1 回滚 commit）"
    exit 1
  fi
fi

# ---- 8. gh release create + upload assets ----
info "[8/8] 创建 GitHub Release"
if [ "$DRY_RUN" = "true" ]; then
  echo "  [dry-run] gh release create $TAG_NAME --title $NEW_VERSION --notes-file $NOTES_FILE"
  if [ -n "$INSTALLER_PATH" ]; then
    echo "  [dry-run] curl POST uploads.github.com/.../assets?name=<file> (binary upload)"
  fi
  echo
  info "DRY-RUN 完成，所有写操作未执行"
else
  if [ -n "$INSTALLER_PATH" ] && [ -f "$INSTALLER_PATH" ]; then
    gh release create "$TAG_NAME" \
      --title "$NEW_VERSION" \
      --notes-file "$NOTES_FILE"

    # 上传 installer：直接 curl GitHub uploads API，避开 gh 在 Windows 上偶尔 hang 的坑
    ASSET_NAME="$(basename "$INSTALLER_PATH")"
    info "上传 installer: $ASSET_NAME ($(du -h "$INSTALLER_PATH" | cut -f1))"
    RELEASE_ID="$(gh release view "$TAG_NAME" --json databaseId --jq '.databaseId')"
    curl -s --max-time 600 -X POST \
      -H "Authorization: token ${GH_TOKEN:-$(gh auth token 2>/dev/null)}" \
      -H "Content-Type: application/octet-stream" \
      --data-binary "@$INSTALLER_PATH" \
      "https://uploads.github.com/repos/RoninQiu/OhMySerialHelper/releases/$RELEASE_ID/assets?name=$ASSET_NAME" \
      -o /tmp/release_upload.json
    if [ -s /tmp/release_upload.json ] && grep -q '"state":"uploaded"' /tmp/release_upload.json; then
      ok "installer 上传成功"
    else
      err "installer 上传可能失败，检查 release assets 页面："
      cat /tmp/release_upload.json
      exit 1
    fi
    rm -f /tmp/release_upload.json
  else
    gh release create "$TAG_NAME" \
      --title "$NEW_VERSION" \
      --notes-file "$NOTES_FILE"
  fi

  echo
  ok "发布完成: https://github.com/RoninQiu/OhMySerialHelper/releases/tag/$TAG_NAME"
fi
