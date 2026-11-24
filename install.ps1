[CmdletBinding()]
param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"
$repo = if ($env:AGYLINE_REPO) { $env:AGYLINE_REPO } else { "CPbianma/AGYCometixLine" }
if (-not $Version) {
    $Version = if ($env:AGYLINE_VERSION) { $env:AGYLINE_VERSION } else { "latest" }
}

if ($env:OS -ne "Windows_NT") {
    throw "This installer is for Windows. Use install.sh on macOS or Linux."
}

$target = "x86_64-pc-windows-msvc"
$asset = "agyline-$target.zip"
$baseUrl = if ($Version -eq "latest") {
    "https://github.com/$repo/releases/latest/download"
} else {
    "https://github.com/$repo/releases/download/$Version"
}

$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("agyline-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
    $archive = Join-Path $tempDir $asset
    $checksums = Join-Path $tempDir "SHA256SUMS"
    Write-Host "Downloading $asset ($Version)..."
    Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset" -OutFile $archive
    Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/SHA256SUMS" -OutFile $checksums

    $expected = $null
    foreach ($line in [IO.File]::ReadAllLines($checksums)) {
        $parts = $line.Trim() -split '\s+'
        if ($parts.Count -ge 2 -and $parts[-1] -eq $asset) {
            $expected = $parts[0]
            break
        }
    }
    if (-not $expected) {
        throw "No checksum published for $asset."
    }

    $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
    if ($actual -ne $expected.ToLowerInvariant()) {
        throw "Checksum mismatch for $asset."
    }

    Expand-Archive -LiteralPath $archive -DestinationPath $tempDir
    $executable = Join-Path $tempDir "agyline.exe"
    & $executable install
    if ($LASTEXITCODE -ne 0) {
        throw "agyline install failed with exit code $LASTEXITCODE."
    }

    $homeDir = if ($env:AGYLINE_HOME) {
        $env:AGYLINE_HOME
    } else {
        Join-Path $HOME ".gemini\antigravity-cli\agyline"
    }
    Write-Host "`nConfigure agyline with:"
    Write-Host "  & `"$(Join-Path $homeDir 'agyline.exe')`" --config"
} finally {
    Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
