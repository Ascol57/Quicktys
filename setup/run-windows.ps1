# Quicktys launcher — Windows.
#
#   powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1
#   ... -Solo         # the app alone, no marketplace server
#   ... -Web          # the frontend alone, in a browser (Vite dev)
#   ... -SkipBuild    # don't rebuild bundles/frontend, just launch
#   ... -Port 9000    # registry server port (default 8000)
param(
  [switch]$Solo,
  [switch]$Web,
  [switch]$SkipBuild,
  [int]$Port = 8000
)
$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $RepoRoot

function Step($m) { Write-Host "`n> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "  [ok] $m" -ForegroundColor Green }
function Info($m) { Write-Host "  $m" -ForegroundColor DarkGray }
function Warn($m) { Write-Host "  [!] $m" -ForegroundColor Yellow }
function Die($m)  { Write-Host "`n[x] $m" -ForegroundColor Red; exit 1 }
function Have($c) { $null -ne (Get-Command $c -ErrorAction SilentlyContinue) }

$env:Path = "$env:Path;$env:USERPROFILE\.cargo\bin"

# The app resolves its data directory from HOME (crates/quicktys-app/src/main.rs).
# Windows sets USERPROFILE, not HOME, and the fallback is the temp directory —
# which would silently throw away profiles and settings between runs.
if (-not $env:HOME) {
  $env:HOME = $env:USERPROFILE
  Info "HOME was unset - pointing it at $env:USERPROFILE for this run"
}

Step "Checking prerequisites"
if (-not (Have cargo)) { Die "Rust not found - run setup\setup-windows.ps1" }
if (-not (Have node))  { Die "Node not found - run setup\setup-windows.ps1" }
if (-not ((rustup target list --installed) -match '^wasm32-wasip2$')) {
  Die "wasm32-wasip2 target missing - run setup\setup-windows.ps1"
}
Ok "toolchain ready"

if (-not (Test-Path "frontend\node_modules")) {
  Step "Installing frontend dependencies (first run)"
  npm --prefix "$RepoRoot\frontend" install
}

if ($Web) {
  Step "Starting the Vite dev server (browser mock)"
  Info "the app's IPC is mocked in the browser - no host, no extensions"
  npm --prefix "$RepoRoot\frontend" run dev
  exit $LASTEXITCODE
}

if (-not $SkipBuild) {
  Step "Building extension bundles (wasm)"
  # build-bundles.sh is a bash script; Git for Windows provides the bash.
  if (-not (Have bash)) {
    Die "no 'bash' on PATH - install Git for Windows (setup\setup-windows.ps1), it ships Git Bash"
  }
  bash scripts/build-bundles.sh
  if ($LASTEXITCODE -ne 0) { Die "bundle build failed" }

  Step "Building the frontend"
  npm --prefix "$RepoRoot\frontend" run build
  if ($LASTEXITCODE -ne 0) { Die "frontend build failed" }
}

if ($Solo) {
  Step "Launching Quicktys (no registry server)"
  Info "extensions come from dist\bundles"
  cargo run -p quicktys-app
  exit $LASTEXITCODE
}

# --- Full mode: pack + sign the catalogue, serve it, run the app against it ---
# The bash dev.sh does this on Unix; this is the same flow, natively.
if (-not ((Test-Path "dist\ca.pub") -and (Test-Path "dist\store.pub"))) {
  Die "development keys missing - run setup\setup-windows.ps1 first"
}

Step "Packing + signing the catalogue"
cargo run -q -p quicktys-pack -- dist\bundles --out dist\pkgs --sign --ca dist\ca.key --encrypt module/pear:pear-pro | Out-Null
cargo run -q -p quicktys-pack -- --issue-license module/pear:pear-pro --store dist\store.key 2>$null |
  Select-String -Pattern '^QTLIC-' | ForEach-Object { $_.Line } | Set-Content "dist\pear.license"
Info "license -> dist\pear.license   (paste it in the app to unlock pear)"

Remove-Item -Recurse -Force "dist\registry" -ErrorAction SilentlyContinue

Step "Building app + registry server"
cargo build -q -p quicktys-regserve -p quicktys-app
if ($LASTEXITCODE -ne 0) { Die "build failed" }

Step "Serving registry on http://127.0.0.1:$Port"
$server = Start-Process -PassThru -NoNewWindow cargo `
  -ArgumentList "run","-q","-p","quicktys-regserve","--","dist\pkgs","--port","$Port","--out","dist\registry"
Start-Sleep -Seconds 1

try {
  Step "Launching Quicktys"
  $env:QUICKTYS_NO_DEV_BUNDLES = "1"
  $env:QUICKTYS_REGISTRY = "http://127.0.0.1:$Port"
  $env:QUICKTYS_CA = (Get-Content "dist\ca.pub" -Raw).Trim()
  $env:QUICKTYS_STORE = (Get-Content "dist\store.pub" -Raw).Trim()
  cargo run -p quicktys-app
} finally {
  Step "Stopping the registry server"
  if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
}
