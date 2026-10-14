<#
.SYNOPSIS
  Orquestador installer for Windows.

.DESCRIPTION
  Downloads the matching installer from the latest GitHub release (or from
  -Version <tag>) and runs it; a UAC prompt is expected. With -Cli it downloads
  the headless `orquestador-proxy.exe` binary instead and installs it under
  $HOME\.orquestador\bin.

.EXAMPLE
  iex "& { $(irm https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.ps1) }"

.EXAMPLE
  iex "& { $(irm https://raw.githubusercontent.com/alecerca/ORCHESTRATOR/main/scripts/install.ps1) } -Cli"

.NOTES
  Needs a published release: https://github.com/alecerca/ORCHESTRATOR/releases
  Full docs: docs/INSTALL.md
#>
[CmdletBinding()]
param(
  [string]$Version = "",
  [switch]$Cli
)

$ErrorActionPreference = "Stop"
$Repo    = "alecerca/ORCHESTRATOR"
$RepoUrl = "https://github.com/$Repo"
$DocsUrl = "$RepoUrl/blob/main/docs/INSTALL.md#build-from-source"

function Info([string]$Message) {
  Write-Host "orquestador-install: $Message"
}

# Fatal error. Throws so the outer handler prints it and stops without calling
# `exit`: this script is meant to be piped to `iex`, and `exit` there would
# close the user's PowerShell session.
function Die([string]$Message) {
  throw $Message
}

try {
  # --- Detect OS/architecture ------------------------------------------------

  $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
  Info "detected: os=windows arch=$arch"

  # --- Ask GitHub for the release -------------------------------------------

  if ($Version) {
    $apiUrl = "https://api.github.com/repos/$Repo/releases/tags/$Version"
  } else {
    $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
  }

  Info "fetching release info ($apiUrl)"
  try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "orquestador-install" }
  } catch {
    $status = $null
    if ($_.Exception.Response) {
      $status = [int]$_.Exception.Response.StatusCode
    }
    if ($status -eq 404) {
      if ($Version) {
        Die "release '$Version' does not exist. See $RepoUrl/releases"
      }
      Write-Host ""
      Write-Host "orquestador-install: there is no published release yet."
      Write-Host ""
      Write-Host "Until the first release is published there is nothing to download."
      Write-Host "Install from source instead:"
      Write-Host "  $DocsUrl"
      Write-Host "or check the releases page later:"
      Write-Host "  $RepoUrl/releases"
      return
    }
    Die "could not reach the GitHub API: $($_.Exception.Message)"
  }

  $tag = $release.tag_name
  if (-not $tag) {
    Die "could not read the release tag from the GitHub API response"
  }
  Info "release: $tag"

  function Get-Asset([string]$Pattern) {
    return $release.assets | Where-Object { $_.name -match $Pattern } | Select-Object -First 1
  }

  # --- Select the asset for this platform ------------------------------------

  if ($Cli) {
    # The Windows CLI asset is x86_64; it also runs on Windows on ARM (emulation).
    $asset = Get-Asset '^orquestador-proxy-windows-x86_64\.exe$'
    if (-not $asset) {
      Die "release $tag has no 'orquestador-proxy-windows-x86_64.exe' asset"
    }
  } else {
    $asset = Get-Asset '_x64[^ ]*\.msi$'
    if (-not $asset) { $asset = Get-Asset '_x64-setup\.exe$' }
    if (-not $asset) { $asset = Get-Asset '\.msi$' }
    if (-not $asset) { $asset = Get-Asset '-setup\.exe$' }
    if (-not $asset) {
      Die "release $tag has no .msi or -setup.exe installer for Windows"
    }
  }

  $assetName = $asset.name
  Info "downloading $assetName"

  # --- Install ---------------------------------------------------------------

  if ($Cli) {
    $binDir = Join-Path $HOME ".orquestador\bin"
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    $dest = Join-Path $binDir "orquestador-proxy.exe"
    $staged = "$dest.new"
    try {
      # Download next to the target, then rename: a failed transfer never
      # clobbers a working install.
      Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $staged -UseBasicParsing
      Move-Item -Force -Path $staged -Destination $dest
    } catch {
      Remove-Item -Force -ErrorAction SilentlyContinue -Path $staged
      Die "download failed: $($asset.browser_download_url)"
    }
    Info "installed orquestador-proxy $tag to $dest"
    Write-Host ""
    Write-Host "Serve (reads .\proxy.json by default):"
    Write-Host "  orquestador-proxy [config.json]"
    Write-Host "Add a user (the password is read from stdin):"
    Write-Host "  orquestador-proxy adduser <name> --role <editor|viewer>"
    Write-Host ""
    Write-Host "Add $binDir to your PATH (per-user) with:"
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"`$env:Path;$binDir`", 'User')"
    return
  }

  $installer = Join-Path $env:TEMP $assetName
  try {
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $installer -UseBasicParsing
  } catch {
    Die "download failed: $($asset.browser_download_url)"
  }

  Info "starting the installer (a UAC prompt is expected)"
  Start-Process $installer
  Info "follow the installer prompts; Orquestador will appear in your Start menu."
} catch {
  [Console]::Error.WriteLine("orquestador-install: error: $($_.Exception.Message)")
}
