# 贡献指南

## PR 要求

在提交 Pull Request 之前，请确保：

### 1. 文件位置正确

分片文件必须放在 `{用户名首字母}/{用户名}.json`

- 用户名 `alice` → `a/alice.json`
- 用户名 `bob` → `b/bob.json`
- 用户名 `my-org` → `m/my-org.json`

### 2. JSON 格式正确

```json
{
  "github_username": "你的GitHub用户名",
  "gpg_key_fingerprints": [
    "完整40位十六进制指纹"
  ],
  "base_pkg_url": "https://你的用户名.github.io/hypo-pkgs",
  "registered_at": "ISO 8601 时间戳"
}
```

校验规则：
- `github_username` 必须与 JSON 文件名和 PR 提交者一致
- `gpg_key_fingerprints` 不能为空，每个指纹必须是 40 位十六进制
- `base_pkg_url` 必须可访问
- `registered_at` 必须是有效的 ISO 8601 时间戳

### 3. 每个 PR 只添加一个开发者

### 4. 确保你的 GPG 公钥可通过以下方式之一获取

- GitHub GPG Keys API（自动）
- `{base_pkg_url}/gpg-key.asc`（well-known 路径）

## CI 自动检查

每次 PR 会触发自动检查：

- ✅ JSON schema 校验
- ✅ 文件名与内容一致性
- ✅ GPG 指纹格式校验
- ✅ base_pkg_url 可访问性

## 审核标准

管理员合并前会检查：
- GPG 公钥真实性（GitHub API / well-known URL）
- base_pkg_url 指向有效的 GitHub Pages
- 不存在恶意注册行为
