# KaiTerm

一款基于 Tauri 2 的桌面 SSH/SFTP 终端工具，将远程终端、文件管理和本地终端整合到一个应用中。

## 功能

- **SSH 终端** — 密码/私钥/SSH Agent 三种认证，多会话 Tab 管理
- **SFTP 文件管理** — 远程文件浏览、上传下载、新建/删除/重命名
- **目录跟随** — SSH 终端 `cd` 自动同步 SFTP 面板，支持 bash/zsh/fish
- **文件传输** — 流式分块上传、4 路并发分段下载、冲突处理、取消与残文件清理
- **本地终端** — 内嵌 CMD/PowerShell/Git Bash/MSYS2/WSL
- **系统信息** — 远程主机 CPU/内存/磁盘/网络/进程/端口一览
- **断线重连** — SSH 断开后自动重连，恢复 Shell 和 SFTP
- **凭据安全** — Windows DPAPI 加密存储密码和私钥路径
- **主题系统** — 3 套应用主题 + 8 套终端配色 + 自定义方案

## 截图

<!-- TODO: 添加截图 -->

## 前置要求

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/) 1.81+
- [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)（Windows 下安装 WebView2，通常系统已自带）

## 快速开始

```bash
# 安装前端依赖
npm install

# 开发模式
npm run tauri dev

# 构建生产包
npm run tauri build
```

## 使用

### 连接远程服务器

1. 点击侧边栏 **+** 按钮新建连接
2. 填写主机、端口、用户名，选择认证方式（密码/私钥/SSH Agent）
3. 保存配置，点击连接
4. 连接成功后自动打开终端和 SFTP 面板

### 快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+F` | 终端搜索 |
| `Ctrl+Tab` | 切换会话 Tab |

## 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | Tauri 2 |
| 后端 | Rust + Tokio + russh |
| 前端 | Vue 3 + Pinia + Vite |
| 终端 | xterm.js + xterm-addon-fit/search/web-links/unicode11/clipboard |
| 加密 | Windows DPAPI |
| 本地 PTY | portable-pty |

## 项目结构

```
src/                          # Vue 3 前端
├── components/               # 组件
├── stores/                   # Pinia 状态管理
└── themes/                   # 主题配置

src-tauri/                    # Rust 后端
└── src/
    ├── lib.rs                # 应用入口 & 命令注册
    ├── models.rs             # 数据模型
    ├── error.rs              # 统一错误类型
    ├── ssh_session.rs        # SSH 连接与会话管理
    ├── sftp_ops.rs           # SFTP 文件操作
    ├── transfers.rs          # 文件上传/下载
    ├── local_term.rs         # Windows 本地终端
    ├── crypto.rs             # 凭据加密
    └── sys_info.rs           # 远程系统信息
```

## 构建

```bash
# 调试构建
npm run tauri build -- --debug

# 发布构建（生成 MSI 安装包）
npm run tauri build
```

构建产物在 `src-tauri/target/release/bundle/` 目录下。

## 开发

```bash
# 仅启动前端 dev server
npm run dev

# 启动 Tauri 开发窗口
npm run tauri dev
```

修改 Rust 代码后需运行 `cargo check` 确保无 warning：

```bash
cd src-tauri && cargo check
```

## License

MIT
