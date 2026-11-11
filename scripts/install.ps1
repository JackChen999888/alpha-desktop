[CmdletBinding()]
param(
    [string]$BuildDirectory,
    [string]$Destination = (Join-Path $env:LOCALAPPDATA 'Programs\Alpha'),
    [switch]$NoShortcuts
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $BuildDirectory) {
    $targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $repo 'target' }
    $BuildDirectory = Join-Path $targetRoot 'release'
}
$source = [IO.Path]::GetFullPath($BuildDirectory)
$destinationPath = [IO.Path]::GetFullPath($Destination)
if ($destinationPath.TrimEnd('\') -eq [IO.Path]::GetPathRoot($destinationPath).TrimEnd('\')) {
    throw 'Choose an application subdirectory, not a drive root.'
}
$files = @{'daozai.exe'='Alpha.exe'; 'desktop-island-event.exe'='desktop-island-event.exe'}
foreach ($name in $files.Keys) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $name) -PathType Leaf)) {
        throw "Build output missing: $name. Build both binaries or pass -BuildDirectory."
    }
}
$installedExe = Join-Path $destinationPath 'Alpha.exe'
$running = Get-Process -Name Alpha -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installedExe }
if ($running) { throw 'Exit the installed Alpha from its tray menu, then run this script again.' }
New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
$backup = Join-Path $destinationPath ('backups\' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
$copies = @(
    @{From=(Join-Path $source 'daozai.exe'); To='Alpha.exe'},
    @{From=(Join-Path $source 'desktop-island-event.exe'); To='desktop-island-event.exe'},
    @{From=(Join-Path $repo 'assets\alpha.ico'); To='Alpha.ico'},
    @{From=(Join-Path $repo 'README.md'); To='README.md'},
    @{From=(Join-Path $repo 'LICENSE'); To='LICENSE'},
    @{From=(Join-Path $repo 'LICENSE-Fly-Island'); To='LICENSE-Fly-Island'},
    @{From=(Join-Path $repo 'THIRD_PARTY_NOTICES.txt'); To='THIRD_PARTY_NOTICES.txt'},
    @{From=(Join-Path $repo 'docs\DEPLOYMENT.md'); To='DEPLOYMENT.md'}
)
foreach ($item in $copies) {
    $to = Join-Path $destinationPath $item.To
    if (Test-Path -LiteralPath $to -PathType Leaf) {
        New-Item -ItemType Directory -Path $backup -Force | Out-Null
        Copy-Item -LiteralPath $to -Destination (Join-Path $backup $item.To)
    }
    Copy-Item -LiteralPath $item.From -Destination $to -Force
    if ((Get-FileHash -LiteralPath $item.From).Hash -ne (Get-FileHash -LiteralPath $to).Hash) {
        throw "Copy verification failed: $($item.To). Existing files are backed up at $backup"
    }
}
if (-not $NoShortcuts) {
    $shell = New-Object -ComObject WScript.Shell
    $folders = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))
    foreach ($folder in $folders) {
        if (-not $folder) { continue }
        New-Item -ItemType Directory -Path $folder -Force | Out-Null
        $linkPath = Join-Path $folder 'Alpha.lnk'
        if (Test-Path -LiteralPath $linkPath -PathType Leaf) {
            New-Item -ItemType Directory -Path $backup -Force | Out-Null
            Copy-Item -LiteralPath $linkPath -Destination (Join-Path $backup ((Split-Path $folder -Leaf) + '-Alpha.lnk'))
        }
        $link = $shell.CreateShortcut($linkPath)
        $link.TargetPath = $installedExe
        $link.Arguments = '--pro'
        $link.WorkingDirectory = $destinationPath
        $link.IconLocation = (Join-Path $destinationPath 'Alpha.ico') + ',0'
        $link.Description = 'Alpha Desktop'
        $link.Save()
    }
}
Write-Output "Installed and hash verified: $installedExe"
Write-Output 'Data defaults to %APPDATA%\Alpha. Existing user data and client Hooks were not changed.'
