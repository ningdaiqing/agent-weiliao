// Agent微聊 - 多 Agent 协作桌面端 v1
// 布局与交互参考微信桌面版；支持检测本地 claude / codex / gemini / qwen CLI，
// 支持私聊与群聊（消息经 stdin 传给 CLI，避免命令行转义问题）。
//
// ============================ 重要踩坑记录（勿删） ============================
//
// 【坑 1｜白屏的真正原因：Tauri 注入的全局名冲突】
//   Tauri v2 会往 window 注入一个名为 `isTauri` 的全局布尔标记（值为 true，不可配置），
//   同时还有 __TAURI__ / __TAURI_INTERNALS__ / __TAURI_IIFE__ / __TAURI_EVENT_PLUGIN_INTERNALS__。
//   若前端内联脚本在顶层声明了同名标识符（例如 function isTauri() {}），经典脚本会在
//   “全局声明实例化”阶段直接抛：
//       SyntaxError: Identifier 'isTauri' has already been declared
//   结果是【整块内联脚本被静默丢弃】：HTML/CSS 正常渲染，但所有靠 JS 渲染的区域全空
//   （现象 = 导航栏在、会话列表和聊天区空白）。注意：node --check / new Function 都发现不了，
//   因为冲突方是运行环境的注入。
//   → 因此前端所有顶层标识符一律加 aw 前缀（awIsTauri / awDlog ...）或改写为 IIFE 内部。
//
// 【坑 2｜WebView2 渲染进程崩溃导致全白】
//   本机 WebView2 153.x 在默认 GPU 加速下，渲染进程会反复崩溃（Crashpad 留下 10MB 转储，
//   页面一个像素都不画）。关闭 GPU 合成并把 GPU 进程并入浏览器进程后稳定。
//   如需恢复默认 GPU 行为：把环境变量 AWL_GPU_ARGS 设为空字符串即可。
//
// 【坑 3｜资源嵌入时机】
//   dist/index.html 的资源是在编译期嵌入 exe 的，改完前端必须重新 cargo build，
//   否则运行的是旧页面。
//
// 【坑 4｜不要用 PowerShell 单引号 here-string 生成前端文件】
//   `r`n 会被原样写进 JS 末尾，造成 "Unexpected identifier 'n'" 语法错误 → 同样白屏。
//   前端文件一律用编辑器/Write 工具写入 UTF-8(无 BOM)。
//
// 【坑 5｜启动时弹黑框】
//   Tauri 官方模板默认写的是 #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//   —— 意思是【release 无控制台、debug 有控制台】（方便开发时看日志）。
//   于是跑 debug 构建（或启动脚本回退到 debug exe）时，Windows 会给这个 console 子系统程序
//   开一个黑色控制台窗口（Win11 默认由 Windows Terminal 承载）。
//   本项目的诊断输出全部走 %TEMP%\agentweiliao-debug.log，不需要控制台，
//   故改为【无条件】windows_subsystem = "windows"，debug/release 都不弹黑框。
//
// ============================================================================

#![windows_subsystem = "windows"]

use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DEFAULT_TIMEOUT_SECS: u64 = 180;

#[derive(Serialize, Clone)]
struct AgentCandidate {
    kind: String,        // claude / codex / gemini / qwen
    display_name: String,
    command: String,     // CLI 可执行名
    installed: bool,
    found_path: String,
}

const CANDIDATES: &[(&str, &str)] = &[
    ("claude", "Claude Code"),
    ("codex", "Codex CLI"),
    ("gemini", "Gemini CLI"),
    ("qwen", "Qwen Code"),
];

/// 扫描 PATH，检测本机安装的 agent CLI
#[tauri::command]
fn detect_agents() -> Vec<AgentCandidate> {
    CANDIDATES
        .iter()
        .map(|(cmd, name)| {
            let out = Command::new("where")
                .arg(cmd)
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            let (installed, found_path) = match out {
                Ok(o) if o.status.success() => {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    let first = s.lines().next().unwrap_or("").to_string();
                    (true, first)
                }
                _ => (false, String::new()),
            };
            AgentCandidate {
                kind: cmd.to_string(),
                display_name: name.to_string(),
                command: cmd.to_string(),
                installed,
                found_path,
            }
        })
        .collect()
}

fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id();
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID"])
        .arg(pid.to_string())
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = child.kill();
}

/// 发消息给 agent，返回回复文本。
/// prompt 通过 stdin 传入（规避中文/换行/特殊字符转义问题）。
#[tauri::command]
fn send_message(
    agent_kind: String,
    agent_command: String,
    agent_name: String,
    prompt: String,
    timeout_secs: Option<u64>,
) -> Result<String, String> {
    // ---- mock agent：本地直接生成 ----
    if agent_kind == "mock" {
        std::thread::sleep(Duration::from_millis(600));
        return Ok(mock_reply(&agent_name, &prompt));
    }

    // ---- 组装真实 CLI 调用参数 ----
    let argv: Vec<String> = match agent_kind.as_str() {
        "claude" => vec!["claude".into(), "-p".into()],
        "codex" => vec![
            "codex".into(),
            "exec".into(),
            "--skip-git-repo-check".into(),
        ],
        "gemini" | "qwen" => vec![agent_kind.clone()],
        // custom：用户自定义命令，按空白拆分，prompt 走 stdin
        _ => agent_command
            .split_whitespace()
            .map(|s| s.to_string())
            .collect(),
    };

    let mut child = Command::new("cmd")
        .arg("/C")
        .args(&argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动 agent 失败: {}", e))?;

    // stdin 写入子线程（写完即关闭，触发 EOF）
    if let Some(stdin) = child.stdin.take() {
        let p = prompt.clone();
        std::thread::spawn(move || {
            let mut stdin = stdin;
            let _ = stdin.write_all(p.as_bytes());
        });
    }

    // stdout / stderr 读取线程（防止管道满导致子进程卡死）
    let stdout_handle = child.stdout.take();
    let t_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut o) = stdout_handle {
            let _ = o.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = child.stderr.take();
    let t_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut e) = stderr_handle {
            let _ = e.read_to_end(&mut buf);
        }
        buf
    });

    // 轮询等待 + 超时控制
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() > deadline {
                    kill_tree(&mut child);
                    return Err(format!(
                        "「{}」响应超时（{} 秒），已终止。",
                        agent_name,
                        timeout.as_secs()
                    ));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(format!("等待进程失败: {}", e)),
        }
    }

    let out_bytes = t_out.join().unwrap_or_default();
    let err_bytes = t_err.join().unwrap_or_default();
    let out = String::from_utf8_lossy(&out_bytes).trim().to_string();
    let err = String::from_utf8_lossy(&err_bytes).trim().to_string();

    if !out.is_empty() {
        Ok(out)
    } else if !err.is_empty() {
        // 截断过长错误
        let shown = if err.chars().count() > 800 {
            let s: String = err.chars().take(800).collect();
            format!("{}\n...(截断)", s)
        } else {
            err
        };
        Err(shown)
    } else {
        Ok("(agent 返回了空内容)".to_string())
    }
}

fn mock_reply(agent_name: &str, prompt: &str) -> String {
    let p = prompt.trim();
    let preview = if p.chars().count() > 60 {
        let s: String = p.chars().take(60).collect();
        format!("{}……", s)
    } else {
        p.to_string()
    };
    let n = prompt.len() % 3;
    let extra = match n {
        0 => "从你的描述里我抓到了两个关键点：一是落地速度，二是可验证的最小闭环。建议先跑通一条链路，再横向铺开。",
        1 => "这个问题我可以拆成三步来看：先明确目标，再定验收标准，最后按优先级推进。需要我展开哪一步？",
        _ => "收到。如果需要我结合具体场景给出执行方案，可以补充更多背景信息（预算、时间、约束条件）。",
    };
    format!(
        "我是「{}」（mock 模拟回复）。你刚才说：「{}」。\n\n{}\n\n（当前机器未检测到对应 CLI 或该 agent 为演示用途，这是本地模拟输出；接入真实 CLI 后即可获得真实回复。）",
        agent_name, preview, extra
    )
}

/// 前端诊断日志：把浏览器端信息写到磁盘，便于排查 WebView2 内的白屏问题
/// （前端 dlog() 调用此命令，日志落在 %TEMP%\agentweiliao-debug.log）
#[tauri::command]
fn debug_log(msg: String) -> Result<(), String> {
    let path = std::env::temp_dir().join("agentweiliao-debug.log");
    let line = format!("[{}] {}\n", ts_now(), msg);
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    f.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

fn ts_now() -> String {
    // 秒级时间戳，避免引入 chrono 依赖
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

fn main() {
    // 【修复】WebView2 渲染进程崩溃规避（详见文件头 坑 2）。
    // 如需恢复默认 GPU 行为：设环境变量 AWL_GPU_ARGS 为空字符串。
    let gpu_args = std::env::var("AWL_GPU_ARGS")
        .unwrap_or_else(|_| "--disable-gpu --disable-gpu-compositing --in-process-gpu".to_string());
    if !gpu_args.trim().is_empty() {
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", &gpu_args);
    }

    tauri::Builder::default()
        .setup(|app| {
            // 窗口在代码里创建（tauri.conf.json 不配 windows 段），便于统一控制尺寸与标题
            let _win = tauri::webview::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Agent微聊")
            .inner_size(1200.0, 800.0)
            .min_inner_size(920.0, 600.0)
            .center()
            .build()
            .map_err(|e| e.to_string())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_agents,
            send_message,
            debug_log
        ])
        .run(tauri::generate_context!())
        .expect("Agent微聊 运行失败");
}
