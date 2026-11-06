param([string]$Executable)
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if(!$Executable){$Executable=Join-Path $project 'src-tauri\target\release\inzoi-better-mod-manager.exe'}
if(!$env:TAURI_SIGNING_PRIVATE_KEY){throw 'Set TAURI_SIGNING_PRIVATE_KEY in the environment before running this local signed-fixture test.'}
$fixture=Join-Path $project ('work\portable-test-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path $fixture | Out-Null
$source=Join-Path $fixture 'Fixture.cs'
@"
using System;using System.IO;using System.Threading;
class Fixture {static void Main(string[] args){if(args.Length>0){Thread.Sleep(2000);return;}File.WriteAllText(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"launched.txt"),"restarted");}}
"@ | Set-Content $source -Encoding utf8
$exe=Join-Path $fixture 'inzoi-better-mod-manager.exe'
& "$env:WINDIR\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /target:winexe "/out:$exe" $source
if($LASTEXITCODE -ne 0){throw 'Fixture compilation failed'}
$version=(Get-Content (Join-Path $project 'package.json') -Raw | ConvertFrom-Json).version
$v=$version.Split('.');$next="$($v[0]).$($v[1]).$([int]$v[2]+1)"
$archive=Join-Path $fixture 'fixture.zip'
Compress-Archive -LiteralPath $exe -DestinationPath $archive
Push-Location $project
try {npm run tauri -- signer sign --app-version $next $archive;if($LASTEXITCODE -ne 0){throw 'Fixture signing failed'}} finally {Pop-Location}
$signature=(Get-Content "$archive.sig" -Raw).Trim()
foreach($mode in @('valid','tampered')){
 $root=Join-Path $fixture $mode
 $stage=Join-Path $root 'data\updates\fixture'
 New-Item -ItemType Directory -Path $stage,(Join-Path $root 'data\library') -Force | Out-Null
 '{}' | Set-Content (Join-Path $root 'portable.json')
 'preserved-data' | Set-Content (Join-Path $root 'data\library\keep.txt')
 $target=Join-Path $root 'inzoi-better-mod-manager.exe'
 [IO.File]::WriteAllText($target,'old-version')
 $package=Join-Path $stage 'package.zip'
 Copy-Item -LiteralPath $archive -Destination $package
 if($mode -eq 'tampered'){$bytes=[IO.File]::ReadAllBytes($package);$bytes[0]=$bytes[0] -bxor 1;[IO.File]::WriteAllBytes($package,$bytes)}
 $parent=Start-Process -FilePath $exe -ArgumentList parent -WindowStyle Hidden -PassThru
 @{root=$root;parent_pid=$parent.Id;version=$next;signature=$signature} | ConvertTo-Json | Set-Content (Join-Path $stage 'plan.json') -Encoding utf8NoBOM
 $helper=Join-Path $stage 'helper.exe'
 Copy-Item -LiteralPath $Executable -Destination $helper
 $process=Start-Process -FilePath $helper -ArgumentList @('--apply-portable-update',('"'+(Join-Path $stage 'plan.json')+'"')) -WindowStyle Hidden -PassThru
 if(!$process.WaitForExit(20000)){throw 'Portable helper timed out'}
 if($mode -eq 'valid'){
  $deadline=[DateTime]::UtcNow.AddSeconds(10)
  while(!(Test-Path (Join-Path $root 'launched.txt')) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
  if(!(Test-Path (Join-Path $root 'launched.txt'))){if(Test-Path (Join-Path $stage 'update-error.txt')){throw (Get-Content (Join-Path $stage 'update-error.txt') -Raw)};throw 'Updated executable did not restart'}
  if([IO.File]::ReadAllText((Join-Path $stage 'previous.exe')) -ne 'old-version'){throw 'Previous executable was not preserved'}
 }else{
  if([IO.File]::ReadAllText($target) -ne 'old-version'){throw 'Tampered update changed executable'}
  if(!(Test-Path (Join-Path $stage 'update-error.txt'))){throw 'Tampered update was not rejected'}
 }
 if((Get-Content (Join-Path $root 'data\library\keep.txt') -Raw).Trim() -ne 'preserved-data'){throw 'Portable update changed library data'}
}
Write-Output 'PASS: signed portable update waits for exit, replaces executable, restarts, preserves data and backup; tampered bytes are rejected.'
