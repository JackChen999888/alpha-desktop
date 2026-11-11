param([Parameter(Mandatory=$true)][string]$NsisCompiler,[string]$WorkDirectory=$env:TEMP)
$ErrorActionPreference='Stop'
$repo=Split-Path -Parent $PSScriptRoot
$id='AlphaPackageTest-'+[Guid]::NewGuid().ToString('N')
$root=Join-Path ([IO.Path]::GetFullPath($WorkDirectory)) $id
$package=Join-Path $root 'fixture'
$installed=Join-Path $root 'Installed Alpha'
$key="HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$id"
New-Item -ItemType Directory -Path $package -Force | Out-Null
# Inert fixture payloads are copied but never executed.
[IO.File]::WriteAllBytes((Join-Path $package 'Alpha.exe'),[byte[]](1,2,3,4))
[IO.File]::WriteAllBytes((Join-Path $package 'desktop-island-event.exe'),[byte[]](5,6,7,8))
Copy-Item -LiteralPath (Join-Path $repo 'assets\alpha.ico') -Destination (Join-Path $package 'Alpha.ico')
foreach($file in @('LICENSE','LICENSE-Fly-Island','THIRD_PARTY_NOTICES.txt')){Copy-Item -LiteralPath (Join-Path $repo $file) -Destination $package}
Set-Content -LiteralPath (Join-Path $package 'README.txt') -Value 'Installer test fixture'
$setup=Join-Path $root 'Alpha-Installer-Test.exe'
& $NsisCompiler /V2 /WX "/DAPP_ID=$id" "/DAPP_NAME=$id" "/DAPP_MUTEX=Local\$id" /DAPP_VERSION=0.1.0 "/DPACKAGE_DIR=$package" "/DOUTPUT_FILE=$setup" (Join-Path $repo 'installer\setup.nsi')
if($LASTEXITCODE -ne 0){throw 'Test installer compilation failed'}
function Invoke-TestProcess([string]$File,[string[]]$Arguments){
    $proc=Start-Process -FilePath $File -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    if(-not $proc.WaitForExit(30000)){Stop-Process -Id $proc.Id;throw 'Test installer timed out'}
    $proc.Refresh()
    return $proc.ExitCode
}
function Assert-Install {
    $reg=Get-ItemProperty -LiteralPath $key
    if($reg.InstallLocation -ne $installed -or $reg.DisplayVersion -ne '0.1.0'){throw 'Install registry mismatch'}
    if($reg.UninstallString -ne ('"'+(Join-Path $installed 'Uninstall.exe')+'"')){throw 'Invalid uninstall registration'}
    foreach($file in @('Alpha.exe','desktop-island-event.exe')){
        if((Get-FileHash -LiteralPath (Join-Path $installed $file)).Hash -ne (Get-FileHash -LiteralPath (Join-Path $package $file)).Hash){throw 'Installed binary hash mismatch'}
    }
    $shell=New-Object -ComObject WScript.Shell
    foreach($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))){
        $link=$shell.CreateShortcut((Join-Path $folder "$id.lnk"))
        if($link.TargetPath -ne (Join-Path $installed 'Alpha.exe') -or $link.Arguments -ne '--pro'){throw 'Shortcut target mismatch'}
    }
}
if((Invoke-TestProcess $setup @('/S',"/D=$installed")) -ne 0){throw 'Initial install failed'}
Assert-Install
New-Item -ItemType Directory -Path (Join-Path $installed 'data') -Force | Out-Null
$sentinel=Join-Path $installed 'data\keep.txt'
Set-Content -LiteralPath $sentinel -Value 'preserve-user-data'
Set-Content -LiteralPath (Join-Path $installed 'keep-user-file.txt') -Value 'preserve-unrelated-file'
Set-Content -LiteralPath (Join-Path $installed 'Alpha.exe') -Value 'old-binary'
if((Invoke-TestProcess $setup @('/S',"/D=$installed")) -ne 0){throw 'Upgrade failed'}
Assert-Install
if((Get-Content -LiteralPath $sentinel -Raw).Trim() -ne 'preserve-user-data'){throw 'Upgrade changed user data'}
$mutex=[Threading.Mutex]::new($false,"Local\$id")
try {
    if((Invoke-TestProcess $setup @('/S',"/D=$installed")) -eq 0){throw 'Running app did not block install'}
    if((Invoke-TestProcess (Join-Path $installed 'Uninstall.exe') @('/S',"_?=$installed")) -eq 0){throw 'Running app did not block uninstall'}
} finally {$mutex.Dispose()}
Assert-Install
$driveRoot=[IO.Path]::GetPathRoot($root)
# NSIS itself may discard a root-only /D and reuse the registered path.
# Both rejection and safe fallback must leave all writes inside this test folder.
$rootBefore=@{}
foreach($name in @('Alpha.exe','desktop-island-event.exe','Uninstall.exe')) {
    $path=Join-Path $driveRoot $name
    $rootBefore[$name]=if(Test-Path -LiteralPath $path){(Get-FileHash -LiteralPath $path).Hash}else{''}
}
$null=Invoke-TestProcess $setup @('/S',"/D=$driveRoot")
Assert-Install
foreach($name in $rootBefore.Keys) {
    $path=Join-Path $driveRoot $name
    $after=if(Test-Path -LiteralPath $path){(Get-FileHash -LiteralPath $path).Hash}else{''}
    if($after -ne $rootBefore[$name]){throw 'Drive root was modified'}
}
# All destructive operations below are confined to this generated test install.
if(-not [IO.Path]::GetFullPath($installed).StartsWith($root+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Test directory escaped its root'}
if((Invoke-TestProcess (Join-Path $installed 'Uninstall.exe') @('/S',"_?=$installed")) -ne 0){throw 'Uninstall failed'}
if(Test-Path -LiteralPath (Join-Path $installed 'Alpha.exe')){throw 'Application remains after uninstall'}
if(Test-Path -LiteralPath $key){throw 'Uninstall registry remains'}
if((Get-Content -LiteralPath $sentinel -Raw).Trim() -ne 'preserve-user-data'){throw 'Uninstall changed data'}
if(-not (Test-Path -LiteralPath (Join-Path $installed 'keep-user-file.txt'))){throw 'Uninstall removed unrelated file'}
foreach($folder in @([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('Programs'))){if(Test-Path -LiteralPath (Join-Path $folder "$id.lnk")){throw 'Shortcut remains'}}
Write-Output 'PASS: install, upgrade, shortcuts, registration, running-app protection, root isolation, uninstall, and data preservation.'
Write-Output "Evidence: $root"
