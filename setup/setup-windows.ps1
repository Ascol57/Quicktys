# Quicktys — one-shot prerequisites install for Windows.
#
#   powershell -ExecutionPolicy Bypass -File setup\setup-windows.ps1
#   powershell -ExecutionPolicy Bypass -File setup\setup-windows.ps1 -AssumeYes
#
# Installs (via winget): the MSVC build tools Rust links against, the Edge
# WebView2 runtime Tauri renders in, Git (for the bash the bundle script needs),
# Node, and Rust + the wasm32-wasip2 target. Then the npm packages and the
# development keys.
param(
  [switch]$AssumeYes
)
$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Step($m) { Write-Host "`n> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "  [ok] $m" -ForegroundColor Green }
function Info($m) { Write-Host "  $m" -ForegroundColor DarkGray }
function Warn($m) { Write-Host "  [!] $m" -ForegroundColor Yellow }
function Die($m)  { Write-Host "`n[x] $m" -ForegroundColor Red; exit 1 }
function Have($c) { $null -ne (Get-Command $c -ErrorAction SilentlyContinue) }

function Ask($m) {
  if ($AssumeYes) { Info "$m -> yes (non-interactive)"; return $true }
  $r = Read-Host "  $m [y/N]"
  return $r -match '^(y|yes)$'
}

# winget installs don't refresh the current session's PATH; do it by hand so a
# single run can install a tool and then use it.
function Refresh-Path {
  $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $user = [Environment]::GetEnvironmentVariable("Path", "User")
  $env:Path = "$machine;$user;$env:USERPROFILE\.cargo\bin"
}

function Winget-Install($id, $label) {
  if (-not (Have winget)) {
    Die "winget is not available. Install '$label' manually, or get App Installer from the Microsoft Store."
  }
  if (Ask "Install $label via winget?") {
    winget install --id $id -e --accept-package-agreements --accept-source-agreements
    Refresh-Path
    Ok "$label installed"
  } else {
    Warn "skipped $label"
  }
}

Write-Host "Quicktys - Windows setup" -ForegroundColor Cyan
Info "repo: $RepoRoot"
Refresh-Path

# --- 1. MSVC build tools (Rust's default toolchain links with them) ----------
Step "MSVC build tools"
if (Have link.exe) {
  Ok "linker found"
} else {
  Warn "not found - Rust's msvc toolchain needs the C++ build tools"
  Winget-Install "Microsoft.VisualStudio.2022.BuildTools" "Visual Studio 2022 Build Tools"
  Info "In the installer, select the 'Desktop development with C++' workload."
}

# --- 2. WebView2 (Tauri's webview on Windows) --------------------------------
Step "Edge WebView2 runtime"
$wv = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\*" -ErrorAction SilentlyContinue |
  Where-Object { $_.pv -and $_.name -like "*WebView2*" }
if ($wv -or (Test-Path "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView")) {
  Ok "installed"
} else {
  Warn "not detected (it ships with Windows 11 and recent Windows 10)"
  Winget-Install "Microsoft.EdgeWebView2Runtime" "WebView2 runtime"
}

# --- 3. Git (its bash runs scripts/build-bundles.sh) -------------------------
Step "Git for Windows"
if (Have git) { Ok (git --version) } else { Winget-Install "Git.Git" "Git for Windows" }
if (-not (Have bash)) {
  Warn "no 'bash' on PATH - scripts/build-bundles.sh needs the Git Bash that ships with Git"
}

# --- 4. Node -----------------------------------------------------------------
Step "Node.js"
if (Have node) { Ok "Node $(node --version)" } else { Winget-Install "OpenJS.NodeJS.LTS" "Node.js LTS" }

# --- 5. Rust + the wasm target ----------------------------------------------
Step "Rust toolchain"
if (Have rustc) { Ok (rustc --version) } else { Winget-Install "Rustlang.Rustup" "Rust (rustup)" }
if (-not (Have rustc)) { Die "Rust is required." }

Step "wasm32-wasip2 target (plugins, modules)"
if ((rustup target list --installed) -match '^wasm32-wasip2$') {
  Ok "already installed"
} else {
  rustup target add wasm32-wasip2
  Ok "installed"
}

# --- 6. Frontend packages ----------------------------------------------------
Step "Frontend dependencies (npm)"
if (Test-Path (Join-Path $RepoRoot "frontend\node_modules")) {
  Ok "frontend\node_modules already present"
} else {
  npm --prefix "$RepoRoot\frontend" install
  Ok "installed"
}

# --- 7. Development keys -----------------------------------------------------
# dist\ is gitignored, so a fresh clone has no keys. See setup/gen-keys.sh for
# what each one is; the same two commands are used here.
Step "Development keys"
Push-Location $RepoRoot
try {
  if ((Test-Path "dist\ca.pub") -and (Test-Path "dist\store.pub")) {
    Ok "already generated"
  } elseif (Ask "Generate them now? (builds the packaging tool first - a few minutes)") {
    New-Item -ItemType Directory -Force -Path "dist" | Out-Null
    cargo build -q -p quicktys-pack

    if (-not (Test-Path "dist\ca.pub")) {
      # The CA is created as part of a --sign run; point it at an empty dir so
      # it makes the keypair without packing anything.
      $empty = Join-Path $env:TEMP ("qt-" + [guid]::NewGuid())
      New-Item -ItemType Directory -Force -Path $empty | Out-Null
      cargo run -q -p quicktys-pack -- $empty --out "$empty\out" --sign --ca "dist\ca.key" | Out-Null
      Remove-Item -Recurse -Force $empty -ErrorAction SilentlyContinue
      Ok "created dist\ca.key + dist\ca.pub"
    }
    if (-not (Test-Path "dist\store.pub")) {
      # --issue-license creates the store keypair and returns before it needs a
      # catalogue, so it doubles as the store-key generator.
      cargo run -q -p quicktys-pack -- --issue-license module/pear:pear-pro --store "dist\store.key" 2>$null |
        Select-String -Pattern '^QTLIC-' | ForEach-Object { $_.Line } | Set-Content "dist\pear.license"
      Ok "created dist\store.key + dist\store.pub"
    }
  } else {
    Info "skipped - run setup\run-windows.ps1 later to generate them"
  }
} finally {
  Pop-Location
}

Step "Ready"
Write-Host @"

  Next:  powershell -ExecutionPolicy Bypass -File setup\run-windows.ps1

  Note: the app stores its data under `$HOME\.local\share\quicktys. Windows does
  not set HOME by default, so run-windows.ps1 points it at your user profile -
  otherwise the data would land in %TEMP% and disappear.
"@
