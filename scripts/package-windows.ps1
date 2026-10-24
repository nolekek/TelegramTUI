# Packaging script for Windows x86_64
$ErrorActionPreference = "Stop"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " Building and Packaging tg-tui for Windows" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

$RootDir = Resolve-Path "$PSScriptRoot\.."
Set-Location $RootDir

# 1. Ensure Windows TDLib DLLs are present
if (-not (Test-Path ".data\tdjson.dll")) {
    Write-Host "[1/4] Fetching Windows TDLib binaries..." -ForegroundColor Yellow
    cargo run -p fetch-tdlib -- windows
} else {
    Write-Host "[1/4] Windows TDLib binaries found in .data/" -ForegroundColor Green
}

# 2. Build release binary for Windows MSVC
Write-Host "[2/4] Compiling tg-tui for Windows (release)..." -ForegroundColor Yellow
cargo build --release -p tg-tui

$BinarySource = "target\release\tg-tui.exe"
if (-not (Test-Path $BinarySource)) {
    throw "Build failed: $BinarySource does not exist."
}

# 3. Assemble distribution folder
$DistDir = "dist\tg-tui-windows-x64"
Write-Host "[3/4] Assembling release folder in $DistDir..." -ForegroundColor Yellow
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

Copy-Item $BinarySource -Destination "$DistDir\tg-tui.exe"
Copy-Item ".data\tdjson.dll" -Destination "$DistDir\"
if (Test-Path ".data\libcrypto-3-x64.dll") { Copy-Item ".data\libcrypto-3-x64.dll" -Destination "$DistDir\" }
if (Test-Path ".data\libssl-3-x64.dll") { Copy-Item ".data\libssl-3-x64.dll" -Destination "$DistDir\" }
if (Test-Path ".data\z.dll") { Copy-Item ".data\z.dll" -Destination "$DistDir\" }

$Readme = @'
# Telegram TUI (Windows x64)

## How to Run:
Double-click `tg-tui.exe` or run from PowerShell/CMD:
```powershell
.\tg-tui.exe
```

All dependencies (tdjson.dll, OpenSSL) are bundled in this folder.
'@
Set-Content -Path "$DistDir\README.txt" -Value $Readme

# 4. Create zip archive
Write-Host "[4/4] Creating dist\tg-tui-windows-x64.zip..." -ForegroundColor Yellow
$ZipPath = "dist\tg-tui-windows-x64.zip"
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
Compress-Archive -Path "$DistDir\*" -DestinationPath $ZipPath

$SizeMb = [math]::Round((Get-Item $ZipPath).Length / 1MB, 2)
Write-Host "SUCCESS! Windows archive created at $ZipPath ($SizeMb MB)" -ForegroundColor Green
