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
    foreach ($entry in @($Entries) + @($pathEntries)) {
        if (-not $entry -or -not (Test-Path $entry) -or $mergedEntries.Contains($entry)) {
            continue
        }
        $mergedEntries.Add($entry)
    }

    $env:PATH = [string]::Join(";", $mergedEntries)
}

function Get-Msys2RootDirectories {
    $rootDirectories = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @(
        $env:MSYS2_LOCATION,
        $(if ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP "msys64" }),
        "C:\msys64"
    )) {
        if (-not $candidateRoot -or $rootDirectories.Contains($candidateRoot)) {
            continue
        }
        $rootDirectories.Add($candidateRoot)
    }

    return $rootDirectories
}

function Get-Msys2BinDirectories {
    param(
        [Parameter(Mandatory = $true)][string]$Target
    )

    $msys2RootDirectories = Get-Msys2RootDirectories
    $targetBinSuffixes = switch ($Target) {
        "x86_64-pc-windows-msvc" { @("ucrt64\bin", "mingw64\bin") }
        "aarch64-pc-windows-msvc" { @("clangarm64\bin") }
        default { @() }
    }

    $binDirectories = [System.Collections.Generic.List[string]]::new()
    foreach ($msys2Root in $msys2RootDirectories) {
        foreach ($binSuffix in @($targetBinSuffixes) + @("usr\bin")) {
            $binDirectories.Add((Join-Path $msys2Root $binSuffix))
        }
    }

    return $binDirectories
}

function Resolve-PkgConfigExecutable {
    $commandNames = @("pkg-config.exe", "pkgconf.exe", "pkg-config", "pkgconf")
    foreach ($commandName in $commandNames) {
        $command = Get-Command $commandName -ErrorAction SilentlyContinue
        if ($command) {
            return $command.Source
        }
    }

    $candidatePaths = [System.Collections.Generic.List[string]]::new()
    foreach ($msys2Root in Get-Msys2RootDirectories) {
        foreach ($candidatePath in @(
            (Join-Path $msys2Root "ucrt64\bin\pkg-config.exe"),
            (Join-Path $msys2Root "ucrt64\bin\pkgconf.exe"),
            (Join-Path $msys2Root "clangarm64\bin\pkg-config.exe"),
            (Join-Path $msys2Root "clangarm64\bin\pkgconf.exe"),
            (Join-Path $msys2Root "mingw64\bin\pkg-config.exe"),
            (Join-Path $msys2Root "mingw64\bin\pkgconf.exe"),
            (Join-Path $msys2Root "usr\bin\pkg-config.exe"),
            (Join-Path $msys2Root "usr\bin\pkgconf.exe")
        )) {
            if (-not $candidatePaths.Contains($candidatePath)) {
                $candidatePaths.Add($candidatePath)
            }
        }
    }
    foreach ($candidatePath in $candidatePaths) {
        if (Test-Path $candidatePath) {
            return $candidatePath
        }
    }

    throw "pkg-config executable not found on PATH or in common MSYS2 locations"
}

function Test-RefMarkerMatches {
    param(
        [Parameter(Mandatory = $true)][string]$MarkerPath,
        [Parameter(Mandatory = $true)][string]$ExpectedRef
    )

    if (-not (Test-Path $MarkerPath)) {
        return $false
    }

    return (Get-Content -Raw -Path $MarkerPath).Trim() -eq $ExpectedRef
}

function Ensure-GitCheckoutAtRef {
    param(
        [Parameter(Mandatory = $true)][string]$RepoPath,
        [Parameter(Mandatory = $true)][string]$RepoUrl,
        [Parameter(Mandatory = $true)][string]$Ref,
        [Parameter(Mandatory = $true)][string]$MarkerPath,
        [string[]]$ResetPaths = @()
    )

    if ((Test-Path (Join-Path $RepoPath ".git")) -and (Test-RefMarkerMatches -MarkerPath $MarkerPath -ExpectedRef $Ref)) {
        return
    }

    foreach ($resetPath in $ResetPaths) {
        if (Test-Path $resetPath) {
            Remove-Item $resetPath -Recurse -Force
        }
    }

    git clone --depth 1 --branch $Ref $RepoUrl $RepoPath
    Set-Content -Path $MarkerPath -Value $Ref
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

    $candidateExecutables = [System.Collections.Generic.List[string]]::new()
    foreach ($msys2Root in Get-Msys2RootDirectories) {
        foreach ($candidateExecutable in @(
            (Join-Path $msys2Root "ucrt64\bin\meson.exe"),
            (Join-Path $msys2Root "clangarm64\bin\meson.exe"),
            (Join-Path $msys2Root "mingw64\bin\meson.exe"),
            (Join-Path $msys2Root "usr\bin\meson.exe")
        )) {
            if (-not $candidateExecutables.Contains($candidateExecutable)) {
                $candidateExecutables.Add($candidateExecutable)
            }
        }
    }
    foreach ($candidateExecutable in $candidateExecutables) {
        if (Test-Path $candidateExecutable) {
            return @{
                Command = $candidateExecutable
                Arguments = @()
            }
        }
    }

    $candidatePythonExecutables = [System.Collections.Generic.List[string]]::new()
    foreach ($msys2Root in Get-Msys2RootDirectories) {
        foreach ($candidatePythonExecutable in @(
            (Join-Path $msys2Root "ucrt64\bin\python.exe"),
            (Join-Path $msys2Root "clangarm64\bin\python.exe"),
            (Join-Path $msys2Root "mingw64\bin\python.exe"),
            (Join-Path $msys2Root "usr\bin\python.exe")
        )) {
            if (-not $candidatePythonExecutables.Contains($candidatePythonExecutable)) {
                $candidatePythonExecutables.Add($candidatePythonExecutable)
            }
        }
    }
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

function Test-GstreamerInstallReady {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot
    )

    return (Test-Path (Join-Path $InstallRoot "lib\pkgconfig\gstreamer-1.0.pc")) -and
        (Test-Path (Join-Path $InstallRoot "bin\gst-launch-1.0.exe"))
}

function Resolve-UxPlayExecutablePath {
    param(
        [Parameter(Mandatory = $true)][string]$BuildRoot
    )

    foreach ($candidatePath in @(
        (Join-Path $BuildRoot "uxplay.exe"),
        (Join-Path $BuildRoot "Release\uxplay.exe")
    )) {
        if (Test-Path $candidatePath) {
            return $candidatePath
        }
    }

    return (Join-Path $BuildRoot "uxplay.exe")
}

function Invoke-PrepareDirectRuntime {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$OutDir,
        [Parameter(Mandatory = $true)][string]$UxPlayExecutable,
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$BeaconScript,
        [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath
    )

    python scripts/prepare_direct_runtime.py `
      --target $Target `
      --out-dir $OutDir `
      --uxplay-path $UxPlayExecutable `
      --gst-root $GstreamerRoot `
      --beacon-script $BeaconScript `
      --beacon-helper-relpath $BeaconHelperRelpath `
      --python-path "python" `
      --uxplay-version $env:UXPLAY_REF `
      --gstreamer-version $env:GSTREAMER_VERSION
}

function Stage-CachedRuntimeIfAvailable {
    param(
        [Parameter(Mandatory = $true)][string]$OutDir,
        [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$UxPlaySrc,
        [Parameter(Mandatory = $true)][string]$UxPlayBuild,
        [Parameter(Mandatory = $true)][string]$GstRoot
    )

    $uxPlayExecutable = Resolve-UxPlayExecutablePath -BuildRoot $UxPlayBuild
    $beaconScript = Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py"
    if (-not (Test-Path $uxPlayExecutable) -or -not (Test-GstreamerInstallReady -InstallRoot $GstRoot) -or -not (Test-Path $beaconScript)) {
        return $false
    }

    Invoke-PrepareDirectRuntime `
      -Target $Target `
      -OutDir $OutDir `
      -UxPlayExecutable $uxPlayExecutable `
      -GstreamerRoot $GstRoot `
      -BeaconScript $beaconScript `
      -BeaconHelperRelpath $BeaconHelperRelpath
    return $true
}

function Repair-GstreamerLibffiFfsUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$Target
    )

    if ($Target -ne "aarch64-pc-windows-msvc") {
        return
    }

    $dlmallocPath = Join-Path $GstreamerRoot "subprojects\libffi\src\dlmalloc.c"
    if (-not (Test-Path $dlmallocPath)) {
        return
    }

    $legacyMacro = "#define compute_bit2idx(X, I) I = ffs(X)-1"
    $patchedMacro = "#define compute_bit2idx(X, I) I = __builtin_ffs(X)-1"
    $dlmallocSource = Get-Content -Raw -Path $dlmallocPath
    if ($dlmallocSource.Contains($patchedMacro) -or -not $dlmallocSource.Contains($legacyMacro)) {
        return
    }

    # clangarm64 does not expose a POSIX ffs declaration here, but the builtin is available.
    $patchedSource = $dlmallocSource.Replace($legacyMacro, $patchedMacro)
    Set-Content -Path $dlmallocPath -Value $patchedSource -NoNewline
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
$UxPlayRefFile = Join-Path $WorkRoot "uxplay.ref"
$GstRoot = Join-Path $WorkRoot "gst-root"
$GstBuild = Join-Path $WorkRoot "gstreamer-build"
$GstSrc = Join-Path $WorkRoot "gstreamer"
$GstreamerRefFile = Join-Path $WorkRoot "gstreamer.ref"

New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

if ($GstreamerSource -ne "source") {
    throw "unsupported Windows GStreamerSource=$GstreamerSource"
}

if (Stage-CachedRuntimeIfAvailable -OutDir $OutDir -BeaconHelperRelpath $BeaconHelperRelpath -Target $Target -UxPlaySrc $UxPlaySrc -UxPlayBuild $UxPlayBuild -GstRoot $GstRoot) {
    return
}

Add-PathEntries -Entries (Get-Msys2BinDirectories -Target $Target)
$pkgConfigExecutable = Resolve-PkgConfigExecutable
$env:PKG_CONFIG = $pkgConfigExecutable
$mesonInvocation = Resolve-MesonInvocation

Write-Host "Using pkg-config executable: $pkgConfigExecutable"
Write-Host "Using Meson command: $($mesonInvocation.Command)"

Ensure-GitCheckoutAtRef -RepoPath $UxPlaySrc -RepoUrl "https://github.com/FDH2/UxPlay.git" -Ref $env:UXPLAY_REF -MarkerPath $UxPlayRefFile -ResetPaths @($UxPlaySrc, $UxPlayBuild)
Ensure-GitCheckoutAtRef -RepoPath $GstSrc -RepoUrl "https://gitlab.freedesktop.org/gstreamer/gstreamer.git" -Ref $env:GSTREAMER_VERSION -MarkerPath $GstreamerRefFile -ResetPaths @($GstSrc, $GstBuild, $GstRoot)
Repair-GstreamerLibffiFfsUsage -GstreamerRoot $GstSrc -Target $Target

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
if (-not (Test-GstreamerInstallReady -InstallRoot $GstRoot)) {
    foreach ($resetPath in @($GstBuild, $GstRoot)) {
        if (Test-Path $resetPath) {
            Remove-Item $resetPath -Recurse -Force
        }
    }
    New-Item -ItemType Directory -Force -Path $GstRoot | Out-Null
    & $mesonInvocation.Command @mesonSetupArgs
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("compile", "-C", $GstBuild))
    & $mesonInvocation.Command @($mesonInvocation.Arguments + @("install", "-C", $GstBuild))
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

$UxPlayExecutable = Resolve-UxPlayExecutablePath -BuildRoot $UxPlayBuild
if (-not (Test-Path $UxPlayExecutable)) {
    if (Test-Path $UxPlayBuild) {
        Remove-Item $UxPlayBuild -Recurse -Force
    }
    cmake @cmakeArgs
    cmake --build $UxPlayBuild --config Release --parallel
    $UxPlayExecutable = Resolve-UxPlayExecutablePath -BuildRoot $UxPlayBuild
}

Invoke-PrepareDirectRuntime `
  -Target $Target `
  -OutDir $OutDir `
  -UxPlayExecutable $UxPlayExecutable `
  -GstreamerRoot $GstRoot `
  -BeaconScript (Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py") `
  -BeaconHelperRelpath $BeaconHelperRelpath
