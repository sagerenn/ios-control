param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$GstreamerSource,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

function Add-PathEntries {
    param(
        [Parameter(Mandatory = $true)][string[]]$Entries
    )

    $pathEntries = @()
    if ($env:PATH) {
        $pathEntries = $env:PATH -split ";"
    }

    $mergedEntries = [System.Collections.Generic.List[string]]::new()
    foreach ($entry in @($pathEntries) + @($Entries)) {
        if (-not $entry -or -not (Test-Path $entry) -or $mergedEntries.Contains($entry)) {
            continue
        }
        $mergedEntries.Add($entry)
    }

    $env:PATH = [string]::Join(";", $mergedEntries)
}

function Get-Msys2BinDirectories {
    param(
        [Parameter(Mandatory = $true)][string]$Target
    )

    $targetBinDirectories = switch ($Target) {
        "x86_64-pc-windows-msvc" { @("C:\msys64\ucrt64\bin", "C:\msys64\mingw64\bin") }
        "aarch64-pc-windows-msvc" { @("C:\msys64\clangarm64\bin") }
        default { @() }
    }

    return @($targetBinDirectories + @("C:\msys64\usr\bin"))
}

function Resolve-PkgConfigExecutable {
    $commandNames = @("pkg-config.exe", "pkgconf.exe", "pkg-config", "pkgconf")
    foreach ($commandName in $commandNames) {
        $command = Get-Command $commandName -ErrorAction SilentlyContinue
        if ($command) {
            return $command.Source
        }
    }

    $candidatePaths = @(
        "C:\msys64\ucrt64\bin\pkg-config.exe",
        "C:\msys64\ucrt64\bin\pkgconf.exe",
        "C:\msys64\clangarm64\bin\pkg-config.exe",
        "C:\msys64\clangarm64\bin\pkgconf.exe",
        "C:\msys64\mingw64\bin\pkg-config.exe",
        "C:\msys64\mingw64\bin\pkgconf.exe",
        "C:\msys64\usr\bin\pkg-config.exe",
        "C:\msys64\usr\bin\pkgconf.exe"
    )
    foreach ($candidatePath in $candidatePaths) {
        if (Test-Path $candidatePath) {
            return $candidatePath
        }
    }

    throw "pkg-config executable not found on PATH or in common MSYS2 locations"
}

function Resolve-MesonInvocation {
    $commandNames = @("meson.exe", "meson")
    foreach ($commandName in $commandNames) {
        $command = Get-Command $commandName -ErrorAction SilentlyContinue
        if ($command) {
            return @{
                Command = $command.Source
                Arguments = @()
            }
        }
    }

    $candidateExecutables = @(
        "C:\msys64\ucrt64\bin\meson.exe",
        "C:\msys64\clangarm64\bin\meson.exe",
        "C:\msys64\mingw64\bin\meson.exe",
        "C:\msys64\usr\bin\meson.exe"
    )
    foreach ($candidateExecutable in $candidateExecutables) {
        if (Test-Path $candidateExecutable) {
            return @{
                Command = $candidateExecutable
                Arguments = @()
            }
        }
    }

    $candidatePythonMesonPairs = @(
        @{
            Python = "C:\msys64\ucrt64\bin\python.exe"
            Meson = "C:\msys64\ucrt64\bin\meson.py"
        },
        @{
            Python = "C:\msys64\clangarm64\bin\python.exe"
            Meson = "C:\msys64\clangarm64\bin\meson.py"
        },
        @{
            Python = "C:\msys64\mingw64\bin\python.exe"
            Meson = "C:\msys64\mingw64\bin\meson.py"
        },
        @{
            Python = "C:\msys64\usr\bin\python.exe"
            Meson = "C:\msys64\usr\bin\meson.py"
        }
    )
    foreach ($candidatePair in $candidatePythonMesonPairs) {
        if ((Test-Path $candidatePair.Python) -and (Test-Path $candidatePair.Meson)) {
            return @{
                Command = $candidatePair.Python
                Arguments = @($candidatePair.Meson)
            }
        }
    }

    throw "meson executable not found on PATH or in common MSYS2 locations"
}

function Invoke-DownloadFile {
    param(
        [Parameter(Mandatory = $true)][string]$Uri,
        [Parameter(Mandatory = $true)][string]$OutFile
    )

    $headers = @{
        "User-Agent" = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36"
    }
    $session = [Microsoft.PowerShell.Commands.WebRequestSession]::new()
    $currentUri = $Uri

    for ($attempt = 0; $attempt -lt 5; $attempt++) {
        if (Test-Path $OutFile) {
            Remove-Item $OutFile -Force
        }

        $response = Invoke-WebRequest `
            -Uri $currentUri `
            -Headers $headers `
            -MaximumRedirection 10 `
            -OutFile $OutFile `
            -PassThru `
            -WebSession $session

        $contentType = [string]$response.Headers["Content-Type"]
        if ((Get-Item $OutFile).Length -gt 0 -and -not $contentType.StartsWith("text/html", [System.StringComparison]::OrdinalIgnoreCase)) {
            return
        }

        $html = Get-Content -Raw -Path $OutFile
        if ($html -match '(?is)<meta[^>]+http-equiv=["'']refresh["''][^>]+content=["''][^;]+;\s*url=([^"''>]+)') {
            $redirectUri = [System.Net.WebUtility]::HtmlDecode($matches[1]).Trim()
            $currentUri = [System.Uri]::new([System.Uri]$currentUri, $redirectUri).AbsoluteUri
            continue
        }
        if ($html -match '(?is)location\.(?:href|replace)\(["'']([^"'']+)') {
            $redirectUri = [System.Net.WebUtility]::HtmlDecode($matches[1]).Trim()
            $currentUri = [System.Uri]::new([System.Uri]$currentUri, $redirectUri).AbsoluteUri
            continue
        }

        throw "failed to download ${Uri}: received HTML instead of a binary payload"
    }

    throw "failed to download $Uri after following anti-bot redirects"
}

function Install-MsiToDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$MsiPath,
        [Parameter(Mandatory = $true)][string]$InstallDir
    )

    $process = Start-Process `
        -FilePath "msiexec.exe" `
        -ArgumentList @("/i", $MsiPath, "/qn", "/norestart", "INSTALLDIR=$InstallDir") `
        -PassThru `
        -Wait

    if ($process.ExitCode -ne 0) {
        throw "msiexec failed for $MsiPath with exit code $($process.ExitCode)"
    }
}

if (-not $env:UXPLAY_REF) {
    throw "UXPLAY_REF must be set"
}
if (-not $env:GSTREAMER_VERSION) {
    throw "GSTREAMER_VERSION must be set"
}

$WorkspaceRoot = (Get-Location).Path
$WorkRoot = Join-Path $WorkspaceRoot ".runtime-cache\$Target"
$UxPlaySrc = Join-Path $WorkRoot "UxPlay"
$UxPlayBuild = Join-Path $WorkRoot "uxplay-build"
$GstRoot = Join-Path $WorkRoot "gst-root"
$GstBuild = Join-Path $WorkRoot "gstreamer-build"
$GstSrc = Join-Path $WorkRoot "gstreamer"

if (Test-Path $WorkRoot) {
    Remove-Item $WorkRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
New-Item -ItemType Directory -Force -Path $GstRoot | Out-Null

git clone --depth 1 --branch $env:UXPLAY_REF https://github.com/FDH2/UxPlay.git $UxPlaySrc

$pkgConfigExecutable = $null

if ($GstreamerSource -eq "download") {
    if ($Target -ne "x86_64-pc-windows-msvc") {
        throw "download is only supported for x86_64-pc-windows-msvc"
    }

    $runtimeInstallerPath = Join-Path $WorkRoot "gstreamer-runtime.msi"
    $develInstallerPath = Join-Path $WorkRoot "gstreamer-devel.msi"
    $baseUri = "https://gstreamer.freedesktop.org/data/pkg/windows/$($env:GSTREAMER_VERSION)/msvc"

    Invoke-DownloadFile -Uri "$baseUri/gstreamer-1.0-msvc-x86_64-$($env:GSTREAMER_VERSION).msi" -OutFile $runtimeInstallerPath
    Invoke-DownloadFile -Uri "$baseUri/gstreamer-1.0-devel-msvc-x86_64-$($env:GSTREAMER_VERSION).msi" -OutFile $develInstallerPath

    Install-MsiToDirectory -MsiPath $runtimeInstallerPath -InstallDir $GstRoot
    Install-MsiToDirectory -MsiPath $develInstallerPath -InstallDir $GstRoot

    foreach ($candidate in @(
        (Join-Path $GstRoot "bin\pkg-config.exe"),
        (Join-Path $GstRoot "bin\pkgconf.exe")
    )) {
        if (Test-Path $candidate) {
            $pkgConfigExecutable = $candidate
            break
        }
    }
} elseif ($GstreamerSource -eq "source") {
    Add-PathEntries -Entries (Get-Msys2BinDirectories -Target $Target)
    $pkgConfigExecutable = Resolve-PkgConfigExecutable
    $env:PKG_CONFIG = $pkgConfigExecutable
    $mesonInvocation = Resolve-MesonInvocation

    Write-Host "Using pkg-config executable: $pkgConfigExecutable"
    Write-Host "Using Meson command: $($mesonInvocation.Command)"

    git clone --depth 1 --branch $env:GSTREAMER_VERSION https://gitlab.freedesktop.org/gstreamer/gstreamer.git $GstSrc
    $mesonSetupArgs = @($mesonInvocation.Arguments + @(
        "setup",
        $GstBuild,
        $GstSrc,
        "--prefix",
        $GstRoot,
        "--libdir",
        "lib",
        "-Ddefault_library=shared",
        "-Dexamples=disabled",
        "-Dtests=disabled",
        "-Ddevtools=enabled",
        "-Ddoc=disabled"
    ))
    & $mesonInvocation.Command @mesonSetupArgs
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("compile", "-C", $GstBuild))
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("install", "-C", $GstBuild))
} else {
    throw "unsupported GStreamerSource=$GstreamerSource"
}

if (-not $pkgConfigExecutable) {
    throw "pkg-config executable could not be resolved for GStreamerSource=$GstreamerSource"
}

$gstPkgConfigPaths = @(
    (Join-Path $GstRoot "lib\pkgconfig"),
    (Join-Path $GstRoot "lib64\pkgconfig"),
    (Join-Path $GstRoot "share\pkgconfig")
) | Where-Object { Test-Path $_ }
if (-not $gstPkgConfigPaths) {
    throw "no GStreamer pkg-config directories found under $GstRoot"
}

$env:PKG_CONFIG_PATH = ((@($gstPkgConfigPaths) + @($env:PKG_CONFIG_PATH)) | Where-Object { $_ }) -join ";"
$env:CMAKE_PREFIX_PATH = (@($GstRoot, $env:CMAKE_PREFIX_PATH) | Where-Object { $_ }) -join ";"
$env:GSTREAMER_ROOT_DIR = Join-Path $GstRoot "lib"
$env:PKG_CONFIG = $pkgConfigExecutable
$env:PATH = "$(Join-Path $GstRoot 'bin');$env:PATH"

$cmakeArgs = @(
    "-S", $UxPlaySrc,
    "-B", $UxPlayBuild,
    "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_PREFIX_PATH=$GstRoot",
    "-DGSTREAMER_ROOT_DIR=$env:GSTREAMER_ROOT_DIR",
    "-DPKG_CONFIG_EXECUTABLE=$pkgConfigExecutable"
)

cmake @cmakeArgs
cmake --build $UxPlayBuild --config Release --parallel

python scripts/prepare_direct_runtime.py `
  --target $Target `
  --out-dir $OutDir `
  --uxplay-path (Join-Path $UxPlayBuild "uxplay.exe") `
  --gst-root $GstRoot `
  --beacon-script (Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py") `
  --beacon-helper-relpath $BeaconHelperRelpath `
  --python-path "python" `
  --uxplay-version $env:UXPLAY_REF `
  --gstreamer-version $env:GSTREAMER_VERSION
