[CmdletBinding()]
param([switch]$SkipBuild,[string]$BuildDirectory,[Parameter(Mandatory=$true)][string]$NsisCompiler,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$version=[regex]::Match((Get-Content (Join-Path $repo 'Cargo.toml') -Raw),'(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
if (-not $version) { throw 'Invalid package version' }
if (-not $OutputDirectory) { $OutputDirectory=Join-Path $repo 'dist' }
$out=[IO.Path]::GetFullPath($OutputDirectory)
$packageName="Alpha-v$version-windows-x64"
$package=Join-Path $out $packageName
if (Test-Path -LiteralPath $package) { throw "Output already exists: $package. Choose a new output directory." }
if (-not $BuildDirectory) {
    $targetRoot=if ($env:CARGO_TARGET_DIR) {$env:CARGO_TARGET_DIR} else {Join-Path $repo 'target'}
    $BuildDirectory=Join-Path $targetRoot 'release'
}
if (-not $SkipBuild) {
    $oldFlags=$env:CARGO_ENCODED_RUSTFLAGS
    try {
        $cargoRoot=if ($env:CARGO_HOME) {$env:CARGO_HOME} else {Join-Path $env:USERPROFILE '.cargo'}
        $flags=@('-C','target-feature=+crt-static',"--remap-path-prefix=$repo=alpha","--remap-path-prefix=$cargoRoot=cargo","--remap-path-prefix=$env:USERPROFILE=user",'-C','link-arg=/PDBALTPATH:%_PDB%')
        if ($env:CARGO_TARGET_DIR) {$flags += "--remap-path-prefix=$env:CARGO_TARGET_DIR=target"}
        $env:CARGO_ENCODED_RUSTFLAGS=$flags -join [char]0x1f
        Push-Location $repo
        try {
            cargo +stable-x86_64-pc-windows-msvc build --locked --release --bin daozai --bin desktop-island-event
            if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
        } finally {Pop-Location}
    } finally {$env:CARGO_ENCODED_RUSTFLAGS=$oldFlags}
}
foreach ($name in @('daozai.exe','desktop-island-event.exe')) {
    $exe=Join-Path $BuildDirectory $name
    & (Join-Path $PSScriptRoot 'check-runtime.ps1') -Executable $exe
    $bytes=[IO.File]::ReadAllBytes($exe)
    foreach ($encoding in @([Text.Encoding]::UTF8,[Text.Encoding]::Unicode)) {
        $text=$encoding.GetString($bytes)
        foreach ($path in @($env:USERPROFILE,$repo,$env:CARGO_TARGET_DIR)) {
            if ($path -and $text.IndexOf($path,[StringComparison]::OrdinalIgnoreCase) -ge 0) {throw 'Binary contains a build-machine path. Rebuild with path remapping.'}
        }
    }
}
New-Item -ItemType Directory -Path $package -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $BuildDirectory 'daozai.exe') -Destination (Join-Path $package 'Alpha.exe')
Copy-Item -LiteralPath (Join-Path $BuildDirectory 'desktop-island-event.exe') -Destination $package
Copy-Item -LiteralPath (Join-Path $repo 'assets\alpha.ico') -Destination (Join-Path $package 'Alpha.ico')
foreach ($name in @('LICENSE','LICENSE-Fly-Island','THIRD_PARTY_NOTICES.txt')) {Copy-Item -LiteralPath (Join-Path $repo $name) -Destination $package}
@"
Alpha Desktop $version - Windows 10/11 x64

直接双击 Alpha.exe 即可运行，无需 Rust、Git、Visual Studio 或管理员权限。
需要打开设置时使用托盘菜单，或运行 Alpha.exe --pro。
数据默认保存在 %APPDATA%\Alpha；免安装版也使用此目录，不随程序目录携带。
DeepSeek 密钥、邮箱和 AI 客户端请在本机自行配置，程序没有内置共享密钥。
升级前先从托盘退出，备份数据目录，再替换程序文件。

项目：https://github.com/chennengjian/alpha-desktop
教程：https://github.com/chennengjian/alpha-desktop/blob/main/docs/DEPLOYMENT.md

此预览版尚未进行商业代码签名。请核对下载来源和 SHA-256，不要关闭安全防护。
"@ | Set-Content -LiteralPath (Join-Path $package 'README.txt') -Encoding utf8
$setup=Join-Path $out "Alpha-Setup-v$version-x64.exe"
& $NsisCompiler /V2 /WX "/DAPP_VERSION=$version" "/DPACKAGE_DIR=$package" "/DOUTPUT_FILE=$setup" (Join-Path $repo 'installer\setup.nsi')
if ($LASTEXITCODE -ne 0) {throw 'Installer compilation failed'}
$zip=Join-Path $out "$packageName.zip"
Compress-Archive -LiteralPath $package -DestinationPath $zip
@($setup,$zip) | ForEach-Object {"$((Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))"} | Set-Content -LiteralPath (Join-Path $out 'SHA256SUMS.txt') -Encoding ascii
Write-Output $setup
Write-Output $zip
