param()

$ErrorActionPreference = "Stop"

function Assert-Command {
  param([string]$Name)

  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

Write-Host "Checking RepoBrain OS toolchain..."

Assert-Command cargo
Assert-Command node
Assert-Command pnpm
Assert-Command python
Assert-Command git

Write-Host "Toolchain looks good."
Write-Host ""
Write-Host "Installing TypeScript workspace dependencies..."
pnpm install
Write-Host ""
Write-Host "Installing Python research dev dependencies..."
python -m pip install -e .\src\python\repobrain_research[dev]
Write-Host ""
Write-Host "Running repo-native doctor..."
cargo xtask doctor
Write-Host ""
Write-Host "Installing repo git hooks..."
cargo xtask install-git-hooks
Write-Host ""
Write-Host "Suggested next steps:"
Write-Host "  cargo xtask fmt"
Write-Host "  cargo xtask policy"
Write-Host "  cargo xtask quality"
Write-Host "  cargo xtask check"
Write-Host "  cargo xtask quickstart"
Write-Host "  pnpm mcp:build"
