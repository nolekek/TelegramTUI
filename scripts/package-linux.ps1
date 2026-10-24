# Packaging script for Linux x86_64
$ErrorActionPreference = "Stop"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " Building and Packaging tg-tui for Linux" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$RootDir = Resolve-Path "$PSScriptRoot\.."
Set-Location $RootDir

# 1. Ensure Linux TDLib shared library is fetched
if (-not (Test-Path ".data\libtdjson.so")) {
    Write-Host "[1/5] Fetching Linux TDLib binary..." -ForegroundColor Yellow
    cargo run -p fetch-tdlib -- linux
} else {
    Write-Host "[1/5] Linux TDLib binary found in .data/libtdjson.so" -ForegroundColor Green
}

# 2. Build release binary for Linux GNU (glibc 2.28 compatibility)
Write-Host "[2/5] Compiling tg-tui for x86_64-unknown-linux-gnu..." -ForegroundColor Yellow
cargo build --target x86_64-unknown-linux-gnu --release -p tg-tui

$BinarySource = "target\x86_64-unknown-linux-gnu\release\tg-tui"
if (-not (Test-Path $BinarySource)) {
    throw "Build failed: $BinarySource does not exist."
}

# 3. Assemble distribution folder
$DistDir = "dist\tg-tui-linux-x86_64"
Write-Host "[3/5] Assembling release folder in $DistDir..." -ForegroundColor Yellow
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

Copy-Item $BinarySource -Destination "$DistDir\tg-tui"
Copy-Item ".data\libtdjson.so" -Destination "$DistDir\libtdjson.so"

$RunScript = @'
#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd "$DIR"
chmod +x ./tg-tui
export LD_LIBRARY_PATH="$DIR:$LD_LIBRARY_PATH"
exec ./tg-tui "$@"
'@
[System.IO.File]::WriteAllText("$DistDir\run.sh", $RunScript.Replace("`r`n", "`n"))

$Readme = @'
# Telegram TUI (Linux x86_64)

## How to Run:
```bash
chmod +x tg-tui run.sh
./tg-tui
# Or using the launcher:
./run.sh
```

## Requirements:
- Linux x86_64 (Fedora, Ubuntu, Debian, Arch, CentOS, openSUSE, etc.)
- OpenSSL 1.1 or 3.x (already installed on virtually all modern distributions)

All dependencies, including TDLib (libtdjson.so), are included in this folder.
'@
[System.IO.File]::WriteAllText("$DistDir\README.md", $Readme.Replace("`r`n", "`n"))

# 4. Verify inside WSL
Write-Host "[4/5] Verifying binary execution in local WSL environment..." -ForegroundColor Yellow
$WslCheck = wsl -d Ubuntu -u root -- bash -c 'rm -rf /tmp/pkg_verify && mkdir -p /tmp/pkg_verify && cp -r /mnt/c/Users/Admin/TelegramRape/dist/tg-tui-linux-x86_64/* /tmp/pkg_verify/ && cd /tmp/pkg_verify && chmod +x ./tg-tui ./run.sh && ./tg-tui >/dev/null 2>&1 & PID=$!; sleep 2; if kill -0 $PID 2>/dev/null; then kill $PID; echo "WSL_OK"; else echo "WSL_FAIL"; fi'
if ($WslCheck -match "WSL_OK") {
    Write-Host "Binary verification in WSL succeeded! TDLib loaded cleanly." -ForegroundColor Green
} else {
    Write-Host "Warning: WSL verification check returned: $WslCheck" -ForegroundColor Yellow
}

# 5. Create tar.gz archive
Write-Host "[5/5] Creating dist\tg-tui-linux-x86_64.tar.gz..." -ForegroundColor Yellow
wsl -d Ubuntu -u root -- bash -c "cd /mnt/c/Users/Admin/TelegramRape/dist && tar -czf tg-tui-linux-x86_64.tar.gz tg-tui-linux-x86_64"

$TarGz = "dist\tg-tui-linux-x86_64.tar.gz"
if (Test-Path $TarGz) {
    $SizeMb = [math]::Round((Get-Item $TarGz).Length / 1MB, 2)
    Write-Host "SUCCESS! Archive created at $TarGz ($SizeMb MB)" -ForegroundColor Green
} else {
    throw "Failed to create tar.gz archive."
}
