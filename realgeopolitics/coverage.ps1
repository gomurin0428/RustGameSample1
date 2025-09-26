$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
Set-Location $projectRoot

$cargo = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
if (-not (Test-Path $cargo)) {
    throw "cargo.exe is missing. Please ensure Rust is installed and PATH is configured."
}

& $cargo coverage
if ($LASTEXITCODE -ne 0) {
    throw "cargo coverage failed (exit code: $LASTEXITCODE)."
}

$lcovPath = Join-Path $projectRoot 'coverage/lcov.info'
& $cargo llvm-cov report --lcov --output-path $lcovPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo llvm-cov report (--lcov) failed (exit code: $LASTEXITCODE)."
}
