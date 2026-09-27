# Agent微聊

> 微信桌面版风格的 **本地多 Agent 协作客户端** —— 把你机器上装的 AI Agent CLI 聚合成一个「微信」。

用 Tauri 2 + 原生 JS 写成，体积小、启动快、无 Electron 包袱。
创建多个 Agent，像聊微信一样跟它们私聊、拉群聊，消息直接经 stdin 喂给你本地的 CLI。

```
┌──────┬────────────────┬──────────────────────────────┐
│      │                │                              │
│ 导航 │   会话列表      │        聊天区                 │
│ 64px │   288px        │   （绿气泡 #95EC69）          │
│      │                │                              │
└──────┴────────────────┴──────────────────────────────┘
```

---

## ✨ 特性

| 能力 | 说明 |
|---|---|
| 🎨 **微信式三栏布局** | 深色导航栏 + 会话列表 + 聊天区，绿气泡，交互对齐微信桌面版 |
| 🤖 **多 Agent 管理** | 自由创建/编辑/删除 Agent，每个 Agent 绑定一个本地 CLI 或走内置 Mock |
| 🔍 **自动检测本地 CLI** | 启动时用 `where` 扫描 `claude` / `codex` / `gemini` / `qwen`，装了就自动接入 |
| 💬 **私聊 + 群聊** | 群聊里多个 Agent 并发回复，各自带名字，互不干扰 |
| 🧩 **Mock Agent 兜底** | 一个 CLI 都没装也能完整体验 UI 与流程 |
| 💾 **本地持久化** | 会话与消息存 localStorage，重启不丢（key 变更时自动迁移旧数据） |
| 📝 **自带诊断日志** | 前端关键生命周期写入 `%TEMP%\agentweiliao-debug.log`，白屏时能自证走到哪一步 |
| 🚀 **零前端构建链** | 单文件 `dist/index.html`，无 npm / 无打包器，改完直接 `cargo build` |

---

## 🛠 技术栈

- **外壳**：Tauri 2（Rust）+ WebView2
- **前端**：单个 `dist/index.html`（原生 HTML/CSS/JS，无框架、无构建步骤）
- **后端**：`src-tauri/src/main.rs`，暴露三个命令：`detect_agents` / `send_message` / `debug_log`

---

## 🚀 快速开始

### 环境要求

- Windows 10/11（WebView2 运行时，Win11 自带）
- [Rust](https://rustup.rs/) stable（MSVC 工具链）
- MSVC Build Tools + Windows SDK

### 构建与运行

**方式一：一键脚本（推荐）**

```bat
git clone https://github.com/ningdaiqing/agent-weiliao.git
cd agent-weiliao
双击 启动Agent微聊.bat
```

脚本会自动检查环境 → 编译 → 启动。首次编译约 3–10 分钟。

**方式二：手动**

```bash
cd src-tauri
cargo build
./target/debug/agent-weiliao.exe
```

> 想直接双击 exe 也可以：`src-tauri\target\debug\agent-weiliao.exe`

### 让 Agent 真正干活

装任一 CLI（以 Claude Code 为例）：

```bash
npm i -g @anthropic-ai/claude-code
```

装好后在应用里进「Agent 管理 → 重新检测」，它会自动出现。

---

## 📁 目录结构

```
agent-weiliao/
├── dist/
│   └── index.html              # 整个前端（三栏布局 + 交互 + 状态管理）
├── src-tauri/
│   ├── src/main.rs             # Rust 后端：检测 CLI、调用 CLI、写日志
│   ├── Cargo.toml
│   ├── build.rs
│   └── tauri.conf.json
├── 启动Agent微聊.bat            # 一键编译+启动
└── .gitignore
```

> ⚠️ `dist/` 是**编译期内嵌**进 exe 的 —— 改完前端必须重新 `cargo build`，否则跑的还是旧页面。

---

## 🧠 设计取舍

**为什么消息走 stdin 而不是命令行参数？**
Node 写的 CLI 要经 `cmd /C` 启动，命令行里的空格、引号、中文、emoji 转义极容易翻车。
走 stdin 可以完全绕过 shell 转义，且原生支持多行 prompt。

**为什么不用框架？**
v1 的目标是「尽快跑通」。一个 HTML 文件省掉了整套 npm / 打包 / 热更新链路，
clone 下来只要有 Rust 就能构建，对使用者更友好。

**为什么消息要并发而不是排队？**
群聊场景下各 Agent 互不依赖，串行等待会让"群里讨论"的体验完全失效。

---

## ⚠️ 踩过的坑（留给后来者）

开发过程中踩了三个足以让人怀疑人生的坑，全部记在 `src-tauri/src/main.rs` 头部注释里：

1. **Tauri 注入的全局名会和白屏关联**
   Tauri v2 会往 `window` 注入布尔全局 `isTauri`（以及 `__TAURI__` / `__TAURI_INTERNALS__` 等）。
   如果前端内联脚本在顶层声明了同名标识符，经典脚本会在**全局声明实例化阶段**直接抛
   `SyntaxError: Identifier 'isTauri' has already been declared`，
   **导致整段脚本被静默丢弃** —— HTML/CSS 照常渲染，但所有 JS 生成的内容全空。
   > 前端顶层标识符一律加前缀（本项目用 `aw`），别用 `isTauri` 这类名字。
   > 另外 `node --check` 和 `new Function()` **都查不出来**，因为冲突方是运行环境注入的。

2. **debug 构建会弹黑框**
   Tauri 官方模板的 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
   意思是「release 无控制台、debug 有控制台」。本项目诊断全部走日志文件，所以改成**无条件**
   `#![windows_subsystem = "windows"]`。
   > 改之前要先确认代码里没有 `println!` —— GUI 子系统下 std 句柄无效，`println!` 会直接 panic。

3. **WebView2 渲染进程崩溃导致全白**
   默认 GPU 加速下渲染进程反复崩溃。启动时注入
   `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu --disable-gpu-compositing --in-process-gpu` 后稳定。

---

## 🗺 Roadmap

- [ ] 流式输出（`claude --output-format stream-json`）
- [ ] 群聊 @ 指定 Agent
- [ ] 消息气泡复制 / 撤回
- [ ] 会话导入导出
- [ ] 无边框窗口 + 自绘标题栏
- [ ] 支持自定义 Agent 启动命令

---

## 📄 License

[MIT](LICENSE)
