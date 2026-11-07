param([string]$MakeNsis = "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe")
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Exercise the real NSIS hook without touching app registrations or prompting
# for UAC. The denied-folder case represents an already attempted elevation.
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("inzoimm-installer-test-" + [guid]::NewGuid())
$hook = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../src-tauri/windows/installer-hooks.nsh'))
$blockedDir = Join-Path $testRoot 'protected install'
$originalAcl = $null
try {
    New-Item -ItemType Directory -Path $testRoot | Out-Null
    $source = @'
Unicode true
RequestExecutionLevel user
SilentInstall silent
!include LogicLib.nsh
!include FileFunc.nsh
!include "@HOOK@"
Name "Installer hook test"
OutFile "@OUTPUT@"
Section
  SetOutPath $INSTDIR
  !insertmacro NSIS_HOOK_PREINSTALL
  FileOpen $0 "$INSTDIR\installed.txt" w
  FileWrite $0 "installed"
  FileClose $0
SectionEnd
'@
    $exe = Join-Path $testRoot 'test-setup.exe'
    $nsi = Join-Path $testRoot 'test.nsi'
    $source.Replace('@HOOK@', $hook).Replace('@OUTPUT@', $exe) | Set-Content -LiteralPath $nsi -Encoding utf8
    & $MakeNsis /V2 $nsi
    if ($LASTEXITCODE -ne 0) { throw 'Failed to compile the installer hook test.' }

    $writableDir = Join-Path $testRoot 'writable install'
    $process = Start-Process -FilePath $exe -ArgumentList "/S /D=$writableDir" -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath "$writableDir/installed.txt")) {
        throw 'A writable install must finish without elevation.'
    }
    if (@(Get-ChildItem -LiteralPath $writableDir).Count -ne 1) { throw 'The write probe left a temporary file behind.' }

    New-Item -ItemType Directory -Path $blockedDir | Out-Null
    $originalAcl = Get-Acl -LiteralPath $blockedDir
    $deniedAcl = Get-Acl -LiteralPath $blockedDir
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent().User
    $rule = [Security.AccessControl.FileSystemAccessRule]::new($identity, 'Write', 'ContainerInherit, ObjectInherit', 'None', 'Deny')
    $deniedAcl.AddAccessRule($rule)
    Set-Acl -LiteralPath $blockedDir -AclObject $deniedAcl
    $process = Start-Process -FilePath $exe -ArgumentList "/S /INZOIMM_ELEVATED /D=$blockedDir" -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -eq 0 -or (Test-Path -LiteralPath "$blockedDir/installed.txt")) {
        throw 'An unwritable elevated retry must fail before installing files.'
    }
    Write-Host 'PASS: writable install, probe cleanup, denied write and elevation retry guard.'
} finally {
    if ($originalAcl) { Set-Acl -LiteralPath $blockedDir -AclObject $originalAcl }
    $resolvedRoot = [IO.Path]::GetFullPath($testRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
    if ([IO.Path]::GetDirectoryName($resolvedRoot) -ne $tempRoot -or [IO.Path]::GetFileName($resolvedRoot) -notlike 'inzoimm-installer-test-*') {
        throw "Refusing to remove unexpected test path: $resolvedRoot"
    }
    if (Test-Path -LiteralPath $resolvedRoot) { Remove-Item -LiteralPath $resolvedRoot -Recurse -Force }
}
