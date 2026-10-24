use std::path::Path;
use std::process::Command;

const TD_VERSION: &str = "v1.8.29";

fn main() {
    println!("cargo:rerun-if-changed=.data");
    println!("cargo:rerun-if-env-changed=TDLIB_PATH");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let needed_file = match target_os.as_str() {
        "windows" => "tdjson.dll",
        "macos" => "libtdjson.dylib",
        _ => "libtdjson.so",
    };

    let data_dir = Path::new(".data");
    if data_dir.join(needed_file).exists() {
        return;
    }

    println!("cargo:warning=TDLib binary ({}) not found in .data/. Fetching automatically...", needed_file);
    let _ = std::fs::create_dir_all(data_dir);

    let (url, zip_name) = match target_os.as_str() {
        "windows" => (
            format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{TD_VERSION}/tdlib.native.windows.x86-64.zip"),
            "tdlib_win.zip",
        ),
        "macos" => (
            format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{TD_VERSION}/tdlib.native.macos.x86-64.zip"),
            "tdlib_mac.zip",
        ),
        _ => (
            format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{TD_VERSION}/tdlib.native.ubuntu-22.04.x86-64.zip"),
            "tdlib_linux.zip",
        ),
    };

    let zip_path = data_dir.join(zip_name);

    // Try downloading with curl (available natively on modern Windows, Linux, macOS)
    let curl_status = Command::new("curl")
        .args(["-L", "-s", "-f", "-o", zip_path.to_str().unwrap(), &url])
        .status();

    let downloaded = curl_status.map(|s| s.success()).unwrap_or(false);

    if downloaded && zip_path.exists() {
        // Extract using tar (built-in on Windows 10/11, Linux, macOS)
        let extract_status = Command::new("tar")
            .args(["-xf", zip_path.to_str().unwrap(), "-C", data_dir.to_str().unwrap()])
            .status();

        if extract_status.map(|s| s.success()).unwrap_or(false) {
            println!("cargo:warning=TDLib binary successfully extracted to .data/");
            let _ = std::fs::remove_file(&zip_path);
            return;
        }

        // Fallback for Windows: powershell Expand-Archive
        if cfg!(target_os = "windows") {
            let ps_status = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("Expand-Archive -Path '{}' -DestinationPath '{}' -Force", zip_path.display(), data_dir.display()),
                ])
                .status();

            if ps_status.map(|s| s.success()).unwrap_or(false) {
                println!("cargo:warning=TDLib binary successfully extracted via PowerShell to .data/");
                let _ = std::fs::remove_file(&zip_path);
                return;
            }
        }
    }

    println!("cargo:warning=Could not automatically fetch TDLib. You can run 'cargo run -p fetch-tdlib' or place {} into .data/ manually.", needed_file);
}
