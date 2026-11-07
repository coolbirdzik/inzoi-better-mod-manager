param([ValidateSet('release','debug')][string]$Profile='release',[switch]$Sign)
$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$config=Get-Content -LiteralPath (Join-Path $project 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
$version=$config.version
$output=Join-Path $project "src-tauri\target\$Profile\bundle\portable"
$source=Join-Path $project "src-tauri\target\$Profile\inzoi-better-mod-manager.exe"
if(!(Test-Path -LiteralPath $source)){throw 'Build the Windows executable before packaging portable.'}
$stage=Join-Path $project ('work\portable-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path $stage,$output -Force | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $stage 'inzoi-better-mod-manager.exe')
'{"portable":true,"format":1}' | Set-Content -LiteralPath (Join-Path $stage 'portable.json') -Encoding UTF8
@"
inZOI Better Mod Manager $version - Windows x64 Portable

Extract this folder to a writable location, then run inzoi-better-mod-manager.exe.
The app keeps its settings, library and browser preferences in the adjacent data folder.
Keep portable.json beside the executable. No installer is required.
Windows WebView2 Runtime must be available on the system.
New versions download, verify, replace the app and restart automatically while keeping data.
"@ | Set-Content -LiteralPath (Join-Path $stage 'README.txt') -Encoding UTF8
$archive=Join-Path $output "inZOI-Better-Mod-Manager_${version}_windows-x64-portable.zip"
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -Force
if($Sign){Push-Location $project;try{pnpm run tauri signer sign --app-version $version $archive;if($LASTEXITCODE -ne 0){throw 'Portable update signing failed'}}finally{Pop-Location}}
# Only this script's generated staging folder is removed; the archive is retained.
if((Resolve-Path -LiteralPath $stage).Path.StartsWith((Join-Path $project 'work\portable-'),[StringComparison]::OrdinalIgnoreCase)){Remove-Item -LiteralPath $stage -Recurse -Force}
Write-Output "Portable ZIP: $archive"
