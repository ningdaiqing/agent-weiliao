fn main() {
    // 前端资源变化时必须触发重新编译，否则 exe 里嵌入的仍是旧 HTML
    println!("cargo:rerun-if-changed=../dist");
    println!("cargo:rerun-if-changed=../dist/index.html");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=icons/icon.ico");

    tauri_build::build()
}
