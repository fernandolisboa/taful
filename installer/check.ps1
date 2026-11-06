# Installs, upgrades and uninstalls Taful with the given setup executable and
# checks that the data folder (%APPDATA%\Taful: database, backups, library)
# comes through every step byte for byte. CI only: it really installs and
# uninstalls Taful for the current user.
#
#   pwsh installer/check.ps1 -Setup target/installer/taful-setup-<version>.exe

param([Parameter(Mandatory)][string]$Setup)

$ErrorActionPreference = 'Stop'
if (-not $env:GITHUB_ACTIONS) { throw 'check.ps1 installs and uninstalls Taful; run it in CI only.' }

$Setup = (Resolve-Path $Setup).Path
$data = Join-Path $env:APPDATA 'Taful'
$appDir = Join-Path $env:LOCALAPPDATA 'Programs\Taful'
$exe = Join-Path $appDir 'taful.exe'
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Taful.lnk'

# Seed the data folder. The smoke run may already have created the database;
# add a backup and a library file so every kind of user data is covered.
New-Item -ItemType Directory -Force (Join-Path $data 'backups'), (Join-Path $data 'library\v1') | Out-Null
Set-Content (Join-Path $data 'backups\taful-check.sqlite') 'backup sentinel'
Set-Content (Join-Path $data 'library\v1\check.tafl') 'library sentinel'
if (-not (Test-Path (Join-Path $data 'taful.sqlite'))) {
  Set-Content (Join-Path $data 'taful.sqlite') 'database sentinel'
}

function Snapshot {
  Get-ChildItem $data -Recurse -File | Sort-Object FullName |
    ForEach-Object { '{0} {1}' -f $_.FullName.Substring($data.Length), (Get-FileHash $_.FullName).Hash }
}
$before = Snapshot

function Assert-DataIntact([string]$step) {
  $after = Snapshot
  $diff = Compare-Object $before $after
  if ($diff) { $diff | Format-Table | Out-String | Write-Host; throw "data folder changed after $step" }
  Write-Host "ok: data folder intact after $step ($($before.Count) files)"
}

function Run([string]$file, [string[]]$arguments) {
  $p = Start-Process -FilePath $file -ArgumentList $arguments -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "$file exited with code $($p.ExitCode)" }
}

$silent = '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'

Run $Setup ($silent + "/LOG=$PWD\install.log")
if (-not (Test-Path $exe)) { throw "install: $exe missing" }
if (-not (Test-Path $shortcut)) { throw "install: Start-menu shortcut $shortcut missing" }
Write-Host "ok: installed to $appDir with Start-menu shortcut"
Assert-DataIntact 'install'

# Upgrade: run the installer over the existing install (same AppId).
Run $Setup ($silent + "/LOG=$PWD\upgrade.log")
if (-not (Test-Path $exe)) { throw "upgrade: $exe missing" }
Assert-DataIntact 'upgrade'

# The uninstaller re-launches itself from %TEMP% and returns at once, so wait
# for the app folder to go away.
Run (Join-Path $appDir 'unins000.exe') $silent
$deadline = (Get-Date).AddSeconds(120)
while ((Test-Path $exe) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
Start-Sleep -Seconds 2
if (Test-Path $exe) { throw "uninstall: $exe still present" }
if (Test-Path $shortcut) { throw "uninstall: Start-menu shortcut still present" }
Write-Host 'ok: uninstalled (app folder and shortcut removed)'
Assert-DataIntact 'uninstall'
