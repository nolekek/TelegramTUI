use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const TD_VERSION: &str = "v1.8.29";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let target = args.get(1).map(|s| s.as_str()).unwrap_or("");

    let target_dir = PathBuf::from(".data");
    fs::create_dir_all(&target_dir)?;

    println!("=== Telegram TDLib Fetcher ===");

    match target {
        "--target=linux" | "linux" => {
            fetch_linux(&target_dir).await?;
        }
        "--target=windows" | "windows" => {
            fetch_windows(&target_dir).await?;
        }
        "--target=macos" | "macos" => {
            fetch_macos(&target_dir).await?;
        }
        "--target=all" | "all" => {
            fetch_windows(&target_dir).await?;
            fetch_linux(&target_dir).await?;
            fetch_macos(&target_dir).await?;
        }
        _ => {
            #[cfg(target_os = "windows")]
            fetch_windows(&target_dir).await?;

            #[cfg(target_os = "macos")]
            fetch_macos(&target_dir).await?;

            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            fetch_linux(&target_dir).await?;
        }
    }

    println!("All requested TDLib binaries are ready in .data/!");
    Ok(())
}

async fn fetch_zip_and_extract(url: &str, target_dir: &Path, expected_extensions: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading from: {}", url);
    let client = reqwest::Client::builder()
        .user_agent("TelegramClientFetcher/1.0")
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(format!("Download failed with status: {}", response.status()).into());
    }

    let bytes = response.bytes().await?;
    println!("Downloaded {} bytes. Extracting...", bytes.len());

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        
        let file_name = Path::new(&name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        if file.is_file() {
            let matches = expected_extensions.iter().any(|ext| file_name.ends_with(ext) || file_name.contains(ext));
            if matches || file_name.contains("tdjson") || file_name.contains("crypto") || file_name.contains("ssl") || file_name == "z.dll" {
                let out_path = target_dir.join(file_name);
                let mut out_file = fs::File::create(&out_path)?;
                std::io::copy(&mut file, &mut out_file)?;
                println!("  Extracted: {}", out_path.display());
            }
        }
    }

    Ok(())
}

async fn fetch_linux(target_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dest = target_dir.join("libtdjson.so");
    if dest.exists() {
        println!("Linux TDLib already exists at {}", dest.display());
        return Ok(());
    }
    println!("Fetching TDLib for Linux (x86_64)...");
    let url = format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{}/tdlib.native.ubuntu-22.04.x86-64.zip", TD_VERSION);
    fetch_zip_and_extract(&url, target_dir, &[".so"]).await?;
    Ok(())
}

async fn fetch_windows(target_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dest = target_dir.join("tdjson.dll");
    if dest.exists() {
        println!("Windows TDLib already exists at {}", dest.display());
        return Ok(());
    }
    println!("Fetching TDLib for Windows (x86_64)...");
    let url = format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{}/tdlib.native.windows.x86-64.zip", TD_VERSION);
    fetch_zip_and_extract(&url, target_dir, &[".dll"]).await?;
    Ok(())
}

async fn fetch_macos(target_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let dest = target_dir.join("libtdjson.dylib");
    if dest.exists() {
        println!("macOS TDLib already exists at {}", dest.display());
        return Ok(());
    }
    println!("Fetching TDLib for macOS (x86_64)...");
    let url = format!("https://github.com/ForNeVeR/tdlib.native/releases/download/{}/tdlib.native.macos.x86-64.zip", TD_VERSION);
    fetch_zip_and_extract(&url, target_dir, &[".dylib"]).await?;
    Ok(())
}
