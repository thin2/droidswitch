param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
foreach ($tool in @('node','npm.cmd','cargo','rustup')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Missing $tool. Install Node.js 24 LTS and Rust MSVC, then reopen PowerShell. See docs/windows-testing.md."
    }
}
$taskHost = & rustc -vV
if ($LASTEXITCODE -ne 0 -or -not ($taskHost -match 'host: .*windows-msvc')) {
    throw 'Use the Windows MSVC Rust toolchain. GNU toolchains are not supported by this build script.'
}
& npm.cmd ci
if ($LASTEXITCODE -ne 0) { throw 'npm ci failed' }
if (-not $SkipTests) {
    & npm.cmd test
    if ($LASTEXITCODE -ne 0) { throw 'Frontend tests failed' }
    & cargo test --locked --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
}
& npm.cmd run tauri build -- --bundles nsis
if ($LASTEXITCODE -ne 0) { throw 'Windows installer build failed' }
Write-Host 'Installer: src-tauri\target\release\bundle\nsis\'
Get-ChildItem 'src-tauri\target\release\bundle\nsis\*.exe' | Select-Object Name, Length
