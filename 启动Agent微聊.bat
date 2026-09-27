@echo off
chcp 65001 >nul
title Agent微聊 - 多 Agent 协作
cd /d "%~dp0"

rem Rust 工具链（rustup 代理损坏时直接用 toolchain 内真 cargo）
set "TCBIN=%USERPROFILE%\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin"
set "PATH=%TCBIN%;%USERPROFILE%\.cargo\bin;%PATH%"

if exist "src-tauri\target\release\agent-weiliao.exe" (
    echo [OK] 启动 Agent微聊（release 版）...
    start "" "src-tauri\target\release\agent-weiliao.exe"
    exit /b 0
)

if exist "src-tauri\target\debug\agent-weiliao.exe" (
    echo [OK] 启动 Agent微聊（debug 版）...
    start "" "src-tauri\target\debug\agent-weiliao.exe"
    exit /b 0
)

echo [..] 首次运行：正在编译（约 3-10 分钟，已配置国内镜像，请耐心等待）...
cd src-tauri
"%TCBIN%\cargo.exe" build 2> build-err.log
if errorlevel 1 (
    echo.
    echo [ERR] 编译失败，错误信息见 src-tauri\build-err.log
    echo 若为"拒绝访问"类错误：请暂时退出 360 安全卫士后重试（实时防护会锁新编译文件）
    pause
    exit /b 1
)
cd ..
if not exist "src-tauri\target\debug\agent-weiliao.exe" (
    echo [ERR] 未找到编译产物，请联系小航排查。
    pause
    exit /b 1
)
echo [OK] 编译完成，启动 Agent微聊...
start "" "src-tauri\target\debug\agent-weiliao.exe"
exit /b 0
