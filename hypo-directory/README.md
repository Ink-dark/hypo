# hypo Official Directory

hypo 官方包目录 —— 去中心化软件分发的信任根。

本仓库托管在 GitHub Pages，hypo 客户端通过 `HYPO_REGISTRY_URL` 指向此处获取已注册开发者的公钥指纹和包索引地址。

## 工作原理

```
你 (开发者)                          hypo 用户
    │                                    │
    ├─ 1. Fork 本仓库                    │
    ├─ 2. 添加你的分片文件                │
    ├─ 3. 提交 PR                        │
    │                                    │
    ├─ PR 通过 CI 校验 ──────────────────┤
    │   - GPG 公钥指纹有效                │
    │   - JSON schema 正确               │
    │   - base_pkg_url 可访问             │
    │                                    │
    ├─ 合并后，用户可见你的包 ───────────┤
    │                                    │
    └─ 之后你自行在你的                  │
       GitHub Pages 上管理包              │
```

## 注册步骤

### 1. 准备 GPG 密钥

```bash
# 生成密钥
gpg --full-generate-key

# 导出公钥指纹
gpg --fingerprint your@email.com
```

### 2. Fork 并添加分片文件

在 `{你的用户名首字母}/` 目录下创建 `{你的 GitHub 用户名}.json`：

```json
{
  "github_username": "alice",
  "gpg_key_fingerprints": [
    "A1B2C3D4E5F67890ABCDEF1234567890ABCDEF12"
  ],
  "base_pkg_url": "https://alice.github.io/hypo-pkgs",
  "registered_at": "2026-07-15T00:00:00Z"
}
```

### 3. 搭建你的包仓库 (GitHub Pages)

在你的 `{username}.github.io` 仓库创建 `hypo-pkgs/` 目录结构：

```
alice.github.io/hypo-pkgs/
├── hypo-index.json              # 你的所有包索引
├── my-tool/
│   └── 1.0.0/
│       ├── hypo-package.json    # 下载信息
│       ├── manifest.toml        # 包元数据
│       └── manifest.toml.sig    # manifest 签名
└── gpg-key.asc                  # 你的 GPG 公钥 (well-known)
```

### 4. 提交 PR

- PR 标题格式：`注册开发者: {你的 GitHub 用户名}`
- 确保只修改了一个分片文件

## 目录结构

```
hypo-directory/
├── registry.json          # 顶层索引（分片列表 + 哈希表 + 公钥轮换）
├── registry.sig           # registry.json 的 GPG 分离签名
├── a/
│   ├── alice.json         # 开发者分片
│   └── alex.json
├── b/
│   └── bob.json
└── ...
```

## 安全模型

1. **根信任** — hypo 二进制硬编码本仓库的官方签名公钥指纹
2. **分片完整性** — `registry.json` 包含各分片文件的 SHA256 哈希
3. **开发者信任** — 分片中声明的 GPG 公钥指纹用于验证包签名
4. **包信任** — `.hypo` 文件整体签名 + manifest 独立签名 + 逐文件哈希

## 链接

- [hypo 客户端](https://github.com/Ink-dark/hypo)
- [包格式规范](https://github.com/Ink-dark/hypo/blob/main/docs/SPEC.md)
