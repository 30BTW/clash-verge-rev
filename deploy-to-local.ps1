# Clash Verge Rev deploy script
# Build dev version and deploy to local install directory F:\Clash\Clash Verge
# User data is stored in %APPDATA%\io.github.clash-verge-rev.clash-verge-rev, not affected
#
# Usage:
#   .\deploy-to-local.ps1
#   or: pnpm run deploy:local
#
# Notes:
# - Only the main binary (clash-verge.exe) is overwritten.
# - Sidecars (verge-mihomo, verge-mihomo-alpha) and resources/ are kept as-is.
# - The first Rust build takes 10-30 minutes; subsequent builds are faster.

param(
    [string]$TargetDir = "F:\Clash\Clash Verge",
    [switch]$Fast
)

$ProjectDir = "D:\ClaudeCode text\clash-verge-rev"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Clash Verge Rev Deploy" -ForegroundColor Cyan
Write-Host "  Target: $TargetDir" -ForegroundColor Cyan
if ($Fast) {
    Write-Host "  Profile: fast-release" -ForegroundColor Cyan
}
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build frontend
Write-Host "[1/3] Building frontend..." -ForegroundColor Yellow
Set-Location $ProjectDir
pnpm run web:build
if ($LASTEXITCODE -ne 0) {
    Write-Host "Frontend build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Frontend build OK!" -ForegroundColor Green
Write-Host ""

# Step 2: Build Rust backend (this embeds the frontend dist)
Write-Host "[2/3] Building Rust backend..." -ForegroundColor Yellow
if ($Fast) {
    cargo build --profile fast-release -p clash-verge
} else {
    cargo build --release -p clash-verge
}
if ($LASTEXITCODE -ne 0) {
    Write-Host "Rust build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "Rust build OK!" -ForegroundColor Green
Write-Host ""

# Step 3: Copy binary to target
Write-Host "[3/3] Copying clash-verge.exe to $TargetDir..." -ForegroundColor Yellow
if (-not (Test-Path $TargetDir)) {
    Write-Host "ERROR: $TargetDir not found!" -ForegroundColor Red
    exit 1
}

# Kill running clash-verge process if any
$process = Get-Process -Name "clash-verge" -ErrorAction SilentlyContinue
if ($process) {
    Write-Host "Stopping running clash-verge process..." -ForegroundColor Yellow
    $process | Stop-Process -Force
    Start-Sleep -Seconds 2
}

if ($Fast) {
    $SourceExe = "$ProjectDir\src-tauri\target\fast-release\clash-verge.exe"
} else {
    $SourceExe = "$ProjectDir\src-tauri\target\release\clash-verge.exe"
}

if (-not (Test-Path $SourceExe)) {
    Write-Host "ERROR: Built binary not found at $SourceExe" -ForegroundColor Red
    exit 1
}

Copy-Item "$SourceExe" "$TargetDir\clash-verge.exe" -Force
Write-Host "Binary copied!" -ForegroundColor Green
Write-Host ""

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Deploy complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Run clash-verge.exe from $TargetDir to start the new version." -ForegroundColor Yellow
Write-Host "User data is in %APPDATA%\io.github.clash-verge-rev.clash-verge-rev, safe." -ForegroundColor Yellow
