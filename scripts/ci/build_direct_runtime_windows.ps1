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

function Test-PythonImportsMeson {
    param(
        [Parameter(Mandatory = $true)][string]$PythonExecutable
    )

    try {
        & $PythonExecutable "-c" "import mesonbuild" 2>$null | Out-Null
        return $true
    } catch {
        return $false
    }
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

    $candidatePythonExecutables = @(
        "C:\msys64\ucrt64\bin\python.exe",
        "C:\msys64\clangarm64\bin\python.exe",
        "C:\msys64\mingw64\bin\python.exe",
        "C:\msys64\usr\bin\python.exe"
    )
    foreach ($candidatePythonExecutable in $candidatePythonExecutables) {
        if (-not (Test-Path $candidatePythonExecutable)) {
            continue
        }

        $candidateMesonScripts = @("meson.py", "meson-script.py")
        foreach ($candidateMesonScript in $candidateMesonScripts) {
            $candidateMesonScriptPath = Join-Path (Split-Path $candidatePythonExecutable -Parent) $candidateMesonScript
            if (Test-Path $candidateMesonScriptPath) {
                return @{
                    Command = $candidatePythonExecutable
                    Arguments = @($candidateMesonScriptPath)
                }
            }
        }
    }

    $pathCandidatePythonExecutables = [System.Collections.Generic.List[string]]::new()
    foreach ($candidatePythonExecutable in $candidatePythonExecutables) {
        if ((Test-Path $candidatePythonExecutable) -and (-not $pathCandidatePythonExecutables.Contains($candidatePythonExecutable))) {
            $pathCandidatePythonExecutables.Add($candidatePythonExecutable)
        }
    }
    foreach ($commandName in @("python.exe", "python")) {
        $command = Get-Command $commandName -ErrorAction SilentlyContinue
        if ($command -and -not $pathCandidatePythonExecutables.Contains($command.Source)) {
            $pathCandidatePythonExecutables.Add($command.Source)
        }
    }
    foreach ($candidatePythonExecutable in $pathCandidatePythonExecutables) {
        if (Test-PythonImportsMeson -PythonExecutable $candidatePythonExecutable) {
            return @{
                Command = $candidatePythonExecutable
                Arguments = @("-m", "mesonbuild.mesonmain")
            }
        }
    }

    throw "meson executable not found on PATH or in common MSYS2 locations"
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

if ($GstreamerSource -ne "source") {
    throw "unsupported Windows GStreamerSource=$GstreamerSource"
}

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
