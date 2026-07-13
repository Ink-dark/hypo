#!/bin/bash
# hypo 官方目录签名脚本
# 每次合并 PR 后，管理员用此脚本重新签名 registry.json

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
REGISTRY="$REPO_ROOT/registry.json"
SIG="$REPO_ROOT/registry.sig"
SIG_OLD="$REPO_ROOT/registry.sig.old"

echo "=== hypo Registry 签名 ==="

# 1. 更新 snapshot_version
VERSION=$(jq '.snapshot_version + 1' "$REGISTRY")
jq ".snapshot_version = $VERSION" "$REGISTRY" > "$REGISTRY.tmp"
mv "$REGISTRY.tmp" "$REGISTRY"
echo "snapshot_version → $VERSION"

# 2. 更新所有分片哈希
echo "计算分片哈希..."
NEW_HASHES="{}"
for shard in $(jq -r '.shards[]' "$REGISTRY"); do
    for file in "$REPO_ROOT/$shard"/*.json; do
        [ -f "$file" ] || continue
        rel="${file#$REPO_ROOT/}"
        hash=$(sha256sum "$file" | cut -d' ' -f1)
        NEW_HASHES=$(echo "$NEW_HASHES" | jq --arg k "$rel" --arg v "sha256:$hash" '.[$k] = $v')
    done
done
jq --argjson hashes "$NEW_HASHES" '.shard_hashes = $hashes' "$REGISTRY" > "$REGISTRY.tmp"
mv "$REGISTRY.tmp" "$REGISTRY"
echo "分片哈希已更新"

# 3. 签名 registry.json
echo "签名 registry.json..."
gpg --detach-sign --armor -o "$SIG" "$REGISTRY"
echo "签名完成: $SIG"

# 4. 更新 snapshot_version 并提交
echo ""
echo "请 commit 并 push:"
echo "  git add registry.json registry.sig"
echo "  git commit -m 'registry: snapshot v$VERSION'"
