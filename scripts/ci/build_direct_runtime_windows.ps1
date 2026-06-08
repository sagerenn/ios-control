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

function Resolve-ObjdumpExecutable {
    $commandNames = @("objdump.exe", "objdump", "llvm-objdump.exe", "llvm-objdump")
    foreach ($commandName in $commandNames) {
        $command = Get-Command $commandName -ErrorAction SilentlyContinue
        if ($command) {
            return $command.Source
        }
    }

    $candidatePaths = [System.Collections.Generic.List[string]]::new()
    foreach ($msys2Root in Get-Msys2RootDirectories) {
        foreach ($candidatePath in @(
            (Join-Path $msys2Root "ucrt64\bin\objdump.exe"),
            (Join-Path $msys2Root "ucrt64\bin\llvm-objdump.exe"),
            (Join-Path $msys2Root "clangarm64\bin\objdump.exe"),
            (Join-Path $msys2Root "clangarm64\bin\llvm-objdump.exe"),
            (Join-Path $msys2Root "mingw64\bin\objdump.exe"),
            (Join-Path $msys2Root "mingw64\bin\llvm-objdump.exe"),
            (Join-Path $msys2Root "usr\bin\objdump.exe"),
            (Join-Path $msys2Root "usr\bin\llvm-objdump.exe")
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

    throw "objdump executable not found on PATH or in common MSYS2 locations"
}

function Resolve-BinaryPathFromDirectories {
    param(
        [Parameter(Mandatory = $true)][string]$BinaryName,
        [Parameter(Mandatory = $true)][string[]]$SearchDirectories
    )

    foreach ($searchDirectory in $SearchDirectories) {
        if (-not $searchDirectory) {
            continue
        }

        $candidatePath = Join-Path $searchDirectory $BinaryName
        if (Test-Path $candidatePath) {
            return (Resolve-Path $candidatePath).Path
        }
    }

    return $null
}

function Get-PeImportedDllNames {
    param(
        [Parameter(Mandatory = $true)][string]$BinaryPath,
        [Parameter(Mandatory = $true)][string]$ObjdumpExecutable
    )

    $dllNames = [System.Collections.Generic.List[string]]::new()
    foreach ($line in (& $ObjdumpExecutable -p $BinaryPath 2>$null)) {
        $match = [regex]::Match($line, '^\s*DLL Name:\s*(?<name>[^\s]+)\s*$')
        if (-not $match.Success) {
            continue
        }

        $dllName = $match.Groups["name"].Value
        if (-not $dllNames.Contains($dllName)) {
            $dllNames.Add($dllName)
        }
    }

    return $dllNames
}

function Resolve-UxPlaySupportPaths {
    param(
        [Parameter(Mandatory = $true)][string]$UxPlayExecutable,
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$Target
    )

    $uxPlayExecutablePath = (Resolve-Path $UxPlayExecutable).Path
    $gstreamerBin = Join-Path $GstreamerRoot "bin"
    $resolvedGstreamerBin = $null
    if (Test-Path $gstreamerBin) {
        $resolvedGstreamerBin = (Resolve-Path $gstreamerBin).Path
    }

    $searchDirectories = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateDirectory in @(
        (Split-Path $uxPlayExecutablePath -Parent),
        $resolvedGstreamerBin
    ) + @(Get-Msys2BinDirectories -Target $Target)) {
        if (-not $candidateDirectory -or -not (Test-Path $candidateDirectory) -or $searchDirectories.Contains($candidateDirectory)) {
            continue
        }
        $searchDirectories.Add($candidateDirectory)
    }

    $objdumpExecutable = Resolve-ObjdumpExecutable
    $supportPaths = [System.Collections.Generic.List[string]]::new()
    $inspectedBinaries = [System.Collections.Generic.List[string]]::new()
    $pendingBinaries = [System.Collections.Generic.Queue[string]]::new()
    $pendingBinaries.Enqueue($uxPlayExecutablePath)

    while ($pendingBinaries.Count -gt 0) {
        $binaryPath = $pendingBinaries.Dequeue()
        if ($inspectedBinaries.Contains($binaryPath)) {
            continue
        }
        $inspectedBinaries.Add($binaryPath)

        foreach ($dllName in Get-PeImportedDllNames -BinaryPath $binaryPath -ObjdumpExecutable $objdumpExecutable) {
            $resolvedPath = Resolve-BinaryPathFromDirectories -BinaryName $dllName -SearchDirectories $searchDirectories
            if (-not $resolvedPath -or $resolvedPath -eq $uxPlayExecutablePath) {
                continue
            }

            if (
                $resolvedGstreamerBin -and
                $resolvedPath.StartsWith($resolvedGstreamerBin, [System.StringComparison]::OrdinalIgnoreCase)
            ) {
                continue
            }

            if (-not $supportPaths.Contains($resolvedPath)) {
                $supportPaths.Add($resolvedPath)
            }
            if (-not $inspectedBinaries.Contains($resolvedPath)) {
                $pendingBinaries.Enqueue($resolvedPath)
            }
        }
    }

    return $supportPaths
}

function Resolve-AdditionalRuntimeSupportPaths {
    param(
        [Parameter(Mandatory = $true)][string]$Target
    )

    $searchDirectories = Get-Msys2BinDirectories -Target $Target
    $supportPaths = [System.Collections.Generic.List[string]]::new()
    foreach ($dllName in @("libbz2-1.dll", "libintl-8.dll", "liblzma-5.dll", "zlib1.dll")) {
        $resolvedPath = Resolve-BinaryPathFromDirectories -BinaryName $dllName -SearchDirectories $searchDirectories
        if ($resolvedPath -and -not $supportPaths.Contains($resolvedPath)) {
            $supportPaths.Add($resolvedPath)
        }
    }

    return $supportPaths
}

function Resolve-Msys2BashExecutable {
    foreach ($msys2Root in Get-Msys2RootDirectories) {
        $candidatePath = Join-Path $msys2Root "usr\bin\bash.exe"
        if (Test-Path $candidatePath) {
            return (Resolve-Path $candidatePath).Path
        }
    }

    $command = Get-Command "bash.exe" -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    throw "MSYS2 bash executable not found"
}

function Convert-PathToMsys2UnixPath {
    param(
        [Parameter(Mandatory = $true)][string]$Path
    )

    if (Test-Path $Path) {
        $resolvedPath = (Resolve-Path $Path).Path
    } else {
        $parentPath = Split-Path $Path -Parent
        $leafName = Split-Path $Path -Leaf
        $resolvedParentPath = (Resolve-Path $parentPath).Path
        $resolvedPath = Join-Path $resolvedParentPath $leafName
    }
    if ($resolvedPath -notmatch '^[A-Za-z]:\\') {
        throw "cannot convert non-drive-qualified path to MSYS2 path: $resolvedPath"
    }

    $drive = $resolvedPath.Substring(0, 1).ToLowerInvariant()
    $tail = $resolvedPath.Substring(2).Replace('\', '/')
    return "/$drive$tail"
}

function Build-DnssdStubLibrary {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$WorkRoot
    )

    $sourcePath = Join-Path $PSScriptRoot "uxplay-compat\dnssd_stub.c"
    if (-not (Test-Path $sourcePath)) {
        throw "dnssd compatibility stub source missing: $sourcePath"
    }

    $outputPath = Join-Path $WorkRoot "dnssd.dll"
    $msystem = switch ($Target) {
        "x86_64-pc-windows-msvc" { "UCRT64" }
        "aarch64-pc-windows-msvc" { "CLANGARM64" }
        default { throw "unsupported target for dnssd compatibility stub: $Target" }
    }
    $toolchainBin = switch ($Target) {
        "x86_64-pc-windows-msvc" { "/ucrt64/bin" }
        "aarch64-pc-windows-msvc" { "/clangarm64/bin" }
    }

    $unixSourcePath = Convert-PathToMsys2UnixPath -Path $sourcePath
    $unixOutputPath = Convert-PathToMsys2UnixPath -Path $outputPath
    $bashExecutable = Resolve-Msys2BashExecutable

    $previousMsystem = $env:MSYSTEM
    $previousChereInvoking = $env:CHERE_INVOKING
    try {
        $env:MSYSTEM = $msystem
        $env:CHERE_INVOKING = "1"
        & $bashExecutable -lc "export PATH=${toolchainBin}:/usr/bin:`$PATH; cc -shared -O2 -Wall -Wextra -o '$unixOutputPath' '$unixSourcePath'"
        if ($LASTEXITCODE -ne 0) {
            throw "failed to build dnssd compatibility stub with MSYS2 $msystem"
        }
    } finally {
        $env:MSYSTEM = $previousMsystem
        $env:CHERE_INVOKING = $previousChereInvoking
    }

    if (-not (Test-Path $outputPath)) {
        throw "dnssd compatibility stub was not produced: $outputPath"
    }

    return (Resolve-Path $outputPath).Path
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
        [string[]]$UxPlaySupportPaths = @(),
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$BeaconScript,
        [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath
    )

    $prepareArgs = @(
        "scripts/prepare_direct_runtime.py",
        "--target", $Target,
        "--out-dir", $OutDir,
        "--uxplay-path", $UxPlayExecutable
    )
    foreach ($supportPath in $UxPlaySupportPaths) {
        $prepareArgs += @("--uxplay-support-path", $supportPath)
    }
    $prepareArgs += @(
        "--gst-root", $GstreamerRoot,
        "--beacon-script", $BeaconScript,
        "--beacon-helper-relpath", $BeaconHelperRelpath,
        "--python-path", "python",
        "--uxplay-version", $env:UXPLAY_REF,
        "--gstreamer-version", $env:GSTREAMER_VERSION
    )

    python @prepareArgs
}

function Stage-CachedRuntimeIfAvailable {
    param(
        [Parameter(Mandatory = $true)][string]$OutDir,
        [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$UxPlaySrc,
        [Parameter(Mandatory = $true)][string]$UxPlayBuild,
        [Parameter(Mandatory = $true)][string]$GstRoot,
        [Parameter(Mandatory = $true)][string]$DnssdSupportPath
    )

    $uxPlayExecutable = Resolve-UxPlayExecutablePath -BuildRoot $UxPlayBuild
    $beaconScript = Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py"
    if (-not (Test-Path $uxPlayExecutable) -or -not (Test-GstreamerInstallReady -InstallRoot $GstRoot) -or -not (Test-Path $beaconScript)) {
        return $false
    }
    $uxPlaySupportPaths = [System.Collections.Generic.List[string]]::new()
    foreach ($supportPath in Resolve-UxPlaySupportPaths `
      -UxPlayExecutable $uxPlayExecutable `
      -GstreamerRoot $GstRoot `
      -Target $Target) {
        if (-not $uxPlaySupportPaths.Contains($supportPath)) {
            $uxPlaySupportPaths.Add($supportPath)
        }
    }
    foreach ($supportPath in Resolve-AdditionalRuntimeSupportPaths -Target $Target) {
        if (-not $uxPlaySupportPaths.Contains($supportPath)) {
            $uxPlaySupportPaths.Add($supportPath)
        }
    }
    if (-not $uxPlaySupportPaths.Contains($DnssdSupportPath)) {
        $uxPlaySupportPaths.Add($DnssdSupportPath)
    }

    Invoke-PrepareDirectRuntime `
      -Target $Target `
      -OutDir $OutDir `
      -UxPlayExecutable $uxPlayExecutable `
      -UxPlaySupportPaths $uxPlaySupportPaths `
      -GstreamerRoot $GstRoot `
      -BeaconScript $beaconScript `
      -BeaconHelperRelpath $BeaconHelperRelpath
    return $true
}

function Repair-UxPlayDnsSdHeaderUsage {
    param(
        [Parameter(Mandatory = $true)][string]$UxPlayRoot
    )

    $compatHeaderSource = Join-Path $PSScriptRoot "uxplay-compat\dns_sd.h"
    if (-not (Test-Path $compatHeaderSource)) {
        throw "dns_sd compatibility header missing: $compatHeaderSource"
    }

    $uxPlayLibRoot = Join-Path $UxPlayRoot "lib"
    $compatHeaderDestination = Join-Path $uxPlayLibRoot "dns_sd.h"
    New-Item -ItemType Directory -Force -Path $uxPlayLibRoot | Out-Null
    Copy-Item -Path $compatHeaderSource -Destination $compatHeaderDestination -Force

    $cmakePath = Join-Path $uxPlayLibRoot "CMakeLists.txt"
    if (-not (Test-Path $cmakePath)) {
        return
    }

    $cmakeSource = Get-Content -Raw -Path $cmakePath
    $newline = if ($cmakeSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $legacyWindowsDnsSdBlock = [string]::Join($newline, @(
        '      message( STATUS "BONJOUR_SDK_HOME " ${BONJOUR_SDK} )',
        '      set(DNSSD "${BONJOUR_SDK}/Lib/x64/dnssd.lib")',
        '      target_link_libraries(airplay ${DNSSD} )',
        '      message( STATUS "dns_sd: using " ${DNSSD} )',
        '      find_path(DNSSD_INCLUDE_DIR dns_sd.h HINTS ${BONJOUR_SDK}/Include )'
    ))
    $patchedWindowsDnsSdBlock = [string]::Join($newline, @(
        '      message( STATUS "BONJOUR_SDK_HOME " ${BONJOUR_SDK} )',
        '      message( STATUS "dns_sd: using runtime-loaded dnssd.dll on Windows" )',
        '      find_path(DNSSD_INCLUDE_DIR dns_sd.h HINTS ${BONJOUR_SDK}/Include ${CMAKE_CURRENT_SOURCE_DIR})'
    ))
    if (
        -not $cmakeSource.Contains($patchedWindowsDnsSdBlock) -and
        $cmakeSource.Contains($legacyWindowsDnsSdBlock)
    ) {
        $cmakeSource = $cmakeSource.Replace($legacyWindowsDnsSdBlock, $patchedWindowsDnsSdBlock)
        Set-Content -Path $cmakePath -Value $cmakeSource -NoNewline
    }
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

function Repair-GstreamerFilesinkFtruncateUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot
    )

    $filesinkPath = Join-Path $GstreamerRoot "subprojects\gstreamer\plugins\elements\gstfilesink.c"
    if (-not (Test-Path $filesinkPath)) {
        return
    }

    $filesinkSource = Get-Content -Raw -Path $filesinkPath
    $newline = if ($filesinkSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $legacyFtruncateMacro = [string]::Join($newline, @(
        "#undef ftruncate",
        "#define ftruncate _chsize"
    ))
    $guardedFtruncateMacro = [string]::Join($newline, @(
        "#undef ftruncate",
        "#if !defined(__MINGW32__)",
        "#define ftruncate _chsize",
        "#endif"
    ))
    if ($filesinkSource.Contains($guardedFtruncateMacro) -or -not $filesinkSource.Contains($legacyFtruncateMacro)) {
        return
    }

    # MinGW's unistd.h already exposes a 64-bit ftruncate when _FILE_OFFSET_BITS=64.
    $patchedFilesinkSource = $filesinkSource.Replace($legacyFtruncateMacro, $guardedFtruncateMacro)
    Set-Content -Path $filesinkPath -Value $patchedFilesinkSource -NoNewline
}

function Repair-GstreamerD3D11WinApiAppHeaderUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot
    )

    $libraryMesonPath = Join-Path $GstreamerRoot "subprojects\gst-plugins-bad\gst-libs\gst\d3d11\meson.build"
    if (Test-Path $libraryMesonPath) {
        $libraryMesonSource = Get-Content -Raw -Path $libraryMesonPath
        $newline = if ($libraryMesonSource.Contains("`r`n")) { "`r`n" } else { "`n" }
        $patchedLibraryMesonSource = $libraryMesonSource

        $legacyLibraryHeaderProbe = [string]::Join($newline, @(
            "d3dcompiler_lib = cc.find_library('d3dcompiler', required: false)",
            "runtimeobject_lib = cc.find_library('runtimeobject', required: false)"
        ))
        $guardedLibraryHeaderProbe = [string]::Join($newline, @(
            "d3dcompiler_lib = cc.find_library('d3dcompiler', required: false)",
            "runtimeobject_lib = cc.find_library('runtimeobject', required: false)",
            "have_winapi_app_xaml_dxinterop_h = cxx.has_header('windows.ui.xaml.media.dxinterop.h', required: false)"
        ))
        if (
            -not $patchedLibraryMesonSource.Contains("have_winapi_app_xaml_dxinterop_h = cxx.has_header('windows.ui.xaml.media.dxinterop.h', required: false)") -and
            $patchedLibraryMesonSource.Contains($legacyLibraryHeaderProbe)
        ) {
            $patchedLibraryMesonSource = $patchedLibraryMesonSource.Replace($legacyLibraryHeaderProbe, $guardedLibraryHeaderProbe)
        }

        $legacyLibraryWinApiAppResult = [string]::Join($newline, @(
            "endif",
            "",
            "if not d3d11_winapi_desktop and not d3d11_winapi_app"
        ))
        $guardedLibraryWinApiAppResult = [string]::Join($newline, @(
            "endif",
            "",
            "d3d11_winapi_app = d3d11_winapi_app and have_winapi_app_xaml_dxinterop_h",
            "",
            "if not d3d11_winapi_desktop and not d3d11_winapi_app"
        ))
        if (
            -not $patchedLibraryMesonSource.Contains("d3d11_winapi_app = d3d11_winapi_app and have_winapi_app_xaml_dxinterop_h") -and
            $patchedLibraryMesonSource.Contains($legacyLibraryWinApiAppResult)
        ) {
            $patchedLibraryMesonSource = $patchedLibraryMesonSource.Replace($legacyLibraryWinApiAppResult, $guardedLibraryWinApiAppResult)
        }

        if ($patchedLibraryMesonSource -ne $libraryMesonSource) {
            Set-Content -Path $libraryMesonPath -Value $patchedLibraryMesonSource -NoNewline
        }
    }

    $mesonPath = Join-Path $GstreamerRoot "subprojects\gst-plugins-bad\sys\d3d11\meson.build"
    if (-not (Test-Path $mesonPath)) {
        return
    }

    $mesonSource = Get-Content -Raw -Path $mesonPath
    $newline = if ($mesonSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $patchedMesonSource = $mesonSource

    $legacyHeaderProbe = [string]::Join($newline, @(
        "runtimeobject_lib = cc.find_library('runtimeobject', required : false)",
        "winmm_lib = cc.find_library('winmm', required: false)"
    ))
    $guardedHeaderProbe = [string]::Join($newline, @(
        "runtimeobject_lib = cc.find_library('runtimeobject', required : false)",
        "winmm_lib = cc.find_library('winmm', required: false)",
        "have_winapi_app_xaml_dxinterop_h = cxx.has_header('windows.ui.xaml.media.dxinterop.h', required: false)"
    ))
    if (
        -not $patchedMesonSource.Contains("have_winapi_app_xaml_dxinterop_h = cxx.has_header('windows.ui.xaml.media.dxinterop.h', required: false)") -and
        $patchedMesonSource.Contains($legacyHeaderProbe)
    ) {
        $patchedMesonSource = $patchedMesonSource.Replace($legacyHeaderProbe, $guardedHeaderProbe)
    }

    $legacyWinApiAppBlock = [string]::Join($newline, @(
        "# if build target is Windows 10 and WINAPI_PARTITION_APP is allowed,",
        "# we can build UWP only modules as well",
        "if d3d11_winapi_app",
        "  d3d11_sources += winapi_app_sources",
        "  extra_dep += [runtimeobject_lib]",
        "  if cc.get_id() == 'msvc' and get_option('b_sanitize') == 'address'",
        "    extra_args += ['/bigobj']",
        "  endif",
        "endif"
    ))
    $guardedWinApiAppBlock = [string]::Join($newline, @(
        "# if build target is Windows 10 and WINAPI_PARTITION_APP is allowed,",
        "# we can build UWP only modules as well",
        "if d3d11_winapi_app and not have_winapi_app_xaml_dxinterop_h and d3d11_winapi_only_app",
        "  error('The d3d11 WinAPI app sources require windows.ui.xaml.media.dxinterop.h')",
        "endif",
        "",
        "if d3d11_winapi_app and have_winapi_app_xaml_dxinterop_h",
        "  d3d11_sources += winapi_app_sources",
        "  extra_dep += [runtimeobject_lib]",
        "  if cc.get_id() == 'msvc' and get_option('b_sanitize') == 'address'",
        "    extra_args += ['/bigobj']",
        "  endif",
        "endif"
    ))
    if (
        -not $patchedMesonSource.Contains("if d3d11_winapi_app and have_winapi_app_xaml_dxinterop_h") -and
        $patchedMesonSource.Contains($legacyWinApiAppBlock)
    ) {
        $patchedMesonSource = $patchedMesonSource.Replace($legacyWinApiAppBlock, $guardedWinApiAppBlock)
    }

    if ($patchedMesonSource -ne $mesonSource) {
        Set-Content -Path $mesonPath -Value $patchedMesonSource -NoNewline
    }
}

function Repair-GstreamerD3D11WinRTCaptureNamespaceUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot
    )

    $winRTCapturePath = Join-Path $GstreamerRoot "subprojects\gst-plugins-bad\sys\d3d11\gstd3d11winrtcapture.cpp"
    if (-not (Test-Path $winRTCapturePath)) {
        return
    }

    $winRTCaptureSource = Get-Content -Raw -Path $winRTCapturePath
    $newline = if ($winRTCaptureSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $patchedWinRTCaptureSource = $winRTCaptureSource
    $legacyNamespaceImport = "using namespace Windows::Graphics::DirectX::Direct3D11;"
    if ($patchedWinRTCaptureSource.Contains($legacyNamespaceImport + $newline)) {
        $patchedWinRTCaptureSource = $patchedWinRTCaptureSource.Replace($legacyNamespaceImport + $newline, "")
    } elseif ($patchedWinRTCaptureSource.Contains($legacyNamespaceImport)) {
        $patchedWinRTCaptureSource = $patchedWinRTCaptureSource.Replace($legacyNamespaceImport, "")
    }

    $legacyInteropAccess = "ComPtr < IDirect3DDxgiInterfaceAccess > access;"
    $portableInteropAccess = "ComPtr < Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess > access;"
    if ($patchedWinRTCaptureSource.Contains($legacyInteropAccess)) {
        $patchedWinRTCaptureSource = $patchedWinRTCaptureSource.Replace($legacyInteropAccess, $portableInteropAccess)
    }

    if ($patchedWinRTCaptureSource -ne $winRTCaptureSource) {
        Set-Content -Path $winRTCapturePath -Value $patchedWinRTCaptureSource -NoNewline
    }
}

function Repair-GstreamerD3D12WgcProbeUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot
    )

    $mesonPath = Join-Path $GstreamerRoot "subprojects\gst-plugins-bad\sys\d3d12\meson.build"
    if (-not (Test-Path $mesonPath)) {
        return
    }

    $mesonSource = Get-Content -Raw -Path $mesonPath
    $newline = if ($mesonSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $patchedMesonSource = $mesonSource

    $legacyWrlInclude = "#include<wrl.h>"
    $portableWrlInclude = [string]::Join($newline, @(
        "#include<wrl.h>",
        "#include<wrl/implements.h>"
    ))
    if (
        -not $patchedMesonSource.Contains("#include<wrl/implements.h>") -and
        $patchedMesonSource.Contains($legacyWrlInclude)
    ) {
        # Keep the WGC capability probe aligned with the RuntimeClass helpers required by
        # gstd3d12graphicscapture.cpp so unsupported MinGW SDKs disable WGC during setup.
        $patchedMesonSource = $patchedMesonSource.Replace($legacyWrlInclude, $portableWrlInclude)
    }

    if ($patchedMesonSource -ne $mesonSource) {
        Set-Content -Path $mesonPath -Value $patchedMesonSource -NoNewline
    }
}

function Repair-GstreamerD3D12GraphicsCaptureHeaderUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot
    )

    $graphicsCapturePath = Join-Path $GstreamerRoot "subprojects\gst-plugins-bad\sys\d3d12\gstd3d12graphicscapture.cpp"
    if (-not (Test-Path $graphicsCapturePath)) {
        return
    }

    $graphicsCaptureSource = Get-Content -Raw -Path $graphicsCapturePath
    $newline = if ($graphicsCaptureSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $patchedGraphicsCaptureSource = $graphicsCaptureSource

    $legacyWrlInclude = "#include <wrl.h>"
    $portableWrlInclude = [string]::Join($newline, @(
        "#include <wrl.h>",
        "#include <wrl/implements.h>"
    ))
    if (
        -not $patchedGraphicsCaptureSource.Contains("#include <wrl/implements.h>") -and
        $patchedGraphicsCaptureSource.Contains($legacyWrlInclude)
    ) {
        $patchedGraphicsCaptureSource = $patchedGraphicsCaptureSource.Replace($legacyWrlInclude, $portableWrlInclude)
    }

    $legacyNamespaceImport = "using namespace Windows::Graphics::DirectX::Direct3D11;"
    if ($patchedGraphicsCaptureSource.Contains($legacyNamespaceImport + $newline)) {
        $patchedGraphicsCaptureSource = $patchedGraphicsCaptureSource.Replace($legacyNamespaceImport + $newline, "")
    } elseif ($patchedGraphicsCaptureSource.Contains($legacyNamespaceImport)) {
        $patchedGraphicsCaptureSource = $patchedGraphicsCaptureSource.Replace($legacyNamespaceImport, "")
    }

    $legacyFrameArrivedHandler = [string]::Join($newline, @(
        "typedef ABI::Windows::Foundation::__FITypedEventHandler_2_Windows__CGraphics__CCapture__CDirect3D11CaptureFramePool_IInspectable_t",
        "    IFrameArrivedHandler;"
    ))
    $portableFrameArrivedHandler = [string]::Join($newline, @(
        "typedef ABI::Windows::Foundation::ITypedEventHandler<ABI::Windows::Graphics::Capture::Direct3D11CaptureFramePool*, IInspectable*>",
        "    IFrameArrivedHandler;"
    ))
    if ($patchedGraphicsCaptureSource.Contains($legacyFrameArrivedHandler)) {
        $patchedGraphicsCaptureSource = $patchedGraphicsCaptureSource.Replace($legacyFrameArrivedHandler, $portableFrameArrivedHandler)
    }

    $legacyItemClosedHandler = [string]::Join($newline, @(
        "typedef ABI::Windows::Foundation::__FITypedEventHandler_2_Windows__CGraphics__CCapture__CGraphicsCaptureItem_IInspectable_t",
        "    IItemClosedHandler;"
    ))
    $portableItemClosedHandler = [string]::Join($newline, @(
        "typedef ABI::Windows::Foundation::ITypedEventHandler<ABI::Windows::Graphics::Capture::GraphicsCaptureItem*, IInspectable*>",
        "    IItemClosedHandler;"
    ))
    if ($patchedGraphicsCaptureSource.Contains($legacyItemClosedHandler)) {
        $patchedGraphicsCaptureSource = $patchedGraphicsCaptureSource.Replace($legacyItemClosedHandler, $portableItemClosedHandler)
    }

    if ($patchedGraphicsCaptureSource -ne $graphicsCaptureSource) {
        Set-Content -Path $graphicsCapturePath -Value $patchedGraphicsCaptureSource -NoNewline
    }
}

function Repair-GstreamerGstCheckThreadDependencyUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$Target
    )

    if ($Target -ne "aarch64-pc-windows-msvc") {
        return
    }

    $mesonPath = Join-Path $GstreamerRoot "subprojects\gstreamer\libs\gst\check\meson.build"
    if (-not (Test-Path $mesonPath)) {
        return
    }

    $mesonSource = Get-Content -Raw -Path $mesonPath
    $newline = if ($mesonSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $gstCheckLibraryPattern = '(?ms)^gst_check = library\(''gstcheck-@0@''\.format\(api_version\),\r?\n.*?^\)\r?\n?'
    $libraryMatch = [regex]::Match($mesonSource, $gstCheckLibraryPattern)
    if (-not $libraryMatch.Success) {
        return
    }

    # clangarm64's winpthreads headers route clock_gettime() through clock_gettime64, but Meson's
    # generic threads dependency does not emit the required winpthread link flag here.
    $desiredLinkDepsBlock = [string]::Join($newline, @(
        "gstcheck_link_deps = [gst_dep]",
        "if host_system == 'windows' and host_machine.cpu_family() == 'aarch64'",
        "  winpthread_dep = cc.find_library('winpthread', required : false)",
        "  if winpthread_dep.found()",
        "    gstcheck_link_deps += winpthread_dep",
        "  else",
        "    gstcheck_link_deps += dependency('threads')",
        "  endif",
        "endif",
        ""
    ))
    $gstCheckDepsPattern = '(?ms)^gstcheck_link_deps = \[gst_dep\]\r?\nif host_system == ''windows'' and host_machine.cpu_family\(\) == ''aarch64''\r?\n.*?^endif\r?\n\r?\n'
    $depsMatch = [regex]::Match($mesonSource, $gstCheckDepsPattern)

    # GStreamer 1.26.x adds extra fields to this library() call, so patch the matched
    # gst_check block in place instead of replacing an outdated literal snippet.
    $patchedLibraryBlock = $libraryMatch.Value
    if ($patchedLibraryBlock.Contains("dependencies : [gst_dep],")) {
        $patchedLibraryBlock = $patchedLibraryBlock.Replace("dependencies : [gst_dep],", "dependencies : gstcheck_link_deps,")
    } elseif (-not $patchedLibraryBlock.Contains("dependencies : gstcheck_link_deps,")) {
        return
    }

    $patchedMesonSource = $mesonSource
    if ($depsMatch.Success) {
        $patchedMesonSource = $patchedMesonSource.Replace($depsMatch.Value, $desiredLinkDepsBlock)
    }

    $replacementLibraryBlock = $patchedLibraryBlock
    if (-not $patchedMesonSource.Contains($desiredLinkDepsBlock)) {
        $replacementLibraryBlock = $desiredLinkDepsBlock + $replacementLibraryBlock
    }
    $patchedMesonSource = $patchedMesonSource.Replace($libraryMatch.Value, $replacementLibraryBlock)

    if ($patchedMesonSource -ne $mesonSource) {
        Set-Content -Path $mesonPath -Value $patchedMesonSource -NoNewline
    }
}

function Repair-GstreamerLibcheckClockGettimeUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [Parameter(Mandatory = $true)][string]$Target
    )

    if ($Target -ne "aarch64-pc-windows-msvc") {
        return
    }

    $libcompatHeaderPath = Join-Path $GstreamerRoot "subprojects\gstreamer\libs\gst\check\libcheck\libcompat\libcompat.h"
    if (Test-Path $libcompatHeaderPath) {
        $libcompatHeaderSource = Get-Content -Raw -Path $libcompatHeaderPath
        $newline = if ($libcompatHeaderSource.Contains("`r`n")) { "`r`n" } else { "`n" }
        $legacyDeclaration = [string]::Join($newline, @(
            "#ifndef HAVE_CLOCK_GETTIME",
            "CK_DLL_EXP int clock_gettime (clockid_t clk_id, struct timespec *ts);",
            "#endif"
        ))
        $guardedDeclaration = [string]::Join($newline, @(
            "#if !defined(HAVE_CLOCK_GETTIME) && !(defined(__MINGW32__) && defined(__aarch64__))",
            "CK_DLL_EXP int clock_gettime (clockid_t clk_id, struct timespec *ts);",
            "#endif"
        ))
        $compatibilityDeclaration = [string]::Join($newline, @(
            "#if defined(__MINGW32__) && defined(__aarch64__) && defined(clock_gettime)",
            "#undef clock_gettime",
            "#endif",
            "#if defined(__MINGW32__) && defined(__aarch64__)",
            "CK_DLL_EXP int clock_gettime (clockid_t clk_id, struct timespec *ts);",
            "#endif"
        ))
        $desiredDeclaration = [string]::Join($newline, @(
            $compatibilityDeclaration,
            $guardedDeclaration
        ))
        $clockGettime64Declaration = [string]::Join($newline, @(
            "#if defined(__MINGW32__) && defined(__aarch64__)",
            "CK_DLL_EXP int clock_gettime64 (clockid_t clk_id, struct timespec *ts);",
            "#endif"
        ))
        $previousAliasedDeclaration = [string]::Join($newline, @(
            $guardedDeclaration,
            $clockGettime64Declaration
        ))
        $badPatchedDeclaration = [string]::Join($newline, @(
            "#ifndef HAVE_CLOCK_GETTIME",
            "#if defined(__MINGW32__) && defined(__aarch64__) && defined(clock_gettime)",
            "#undef clock_gettime",
            "#endif",
            "CK_DLL_EXP int clock_gettime (clockid_t clk_id, struct timespec *ts);",
            $clockGettime64Declaration,
            "#endif"
        ))

        # clangarm64's pthread_time.h rewrites clock_gettime to clock_gettime64, but libcheck needs
        # the plain clock_gettime symbol name while still keeping its fallback disabled there.
        $patchedLibcompatHeaderSource = $libcompatHeaderSource
        if ($patchedLibcompatHeaderSource.Contains($badPatchedDeclaration)) {
            $patchedLibcompatHeaderSource = $patchedLibcompatHeaderSource.Replace($badPatchedDeclaration, $desiredDeclaration)
        } elseif ($patchedLibcompatHeaderSource.Contains($previousAliasedDeclaration)) {
            $patchedLibcompatHeaderSource = $patchedLibcompatHeaderSource.Replace($previousAliasedDeclaration, $desiredDeclaration)
        } elseif ($patchedLibcompatHeaderSource.Contains($guardedDeclaration)) {
            $patchedLibcompatHeaderSource = $patchedLibcompatHeaderSource.Replace($guardedDeclaration, $desiredDeclaration)
        } elseif ($patchedLibcompatHeaderSource.Contains($legacyDeclaration)) {
            $patchedLibcompatHeaderSource = $patchedLibcompatHeaderSource.Replace($legacyDeclaration, $desiredDeclaration)
        }
        $clockGettime64DeclarationPattern = '(?ms)^#if defined\(__MINGW32__\) && defined\(__aarch64__\)\r?\nCK_DLL_EXP int clock_gettime64 \(clockid_t clk_id, struct [^\r\n]*\*ts\);\r?\n#endif\r?\n?'
        $clockGettime64DeclarationLinePattern = '(?m)^CK_DLL_EXP int clock_gettime64 \(clockid_t clk_id, struct [^\r\n]*\*ts\);\r?\n?'
        $patchedLibcompatHeaderSource = [regex]::Replace($patchedLibcompatHeaderSource, $clockGettime64DeclarationPattern, "")
        $patchedLibcompatHeaderSource = [regex]::Replace($patchedLibcompatHeaderSource, $clockGettime64DeclarationLinePattern, "")

        if ($patchedLibcompatHeaderSource -ne $libcompatHeaderSource) {
            Set-Content -Path $libcompatHeaderPath -Value $patchedLibcompatHeaderSource -NoNewline
        }
    }

    $clockGettimePath = Join-Path $GstreamerRoot "subprojects\gstreamer\libs\gst\check\libcheck\libcompat\clock_gettime.c"
    if (-not (Test-Path $clockGettimePath)) {
        return
    }

    $clockGettimeSource = Get-Content -Raw -Path $clockGettimePath
    $newline = if ($clockGettimeSource.Contains("`r`n")) { "`r`n" } else { "`n" }
    $guard = "#if !(defined(__MINGW32__) && defined(__aarch64__))"
    $clockGettime64Wrapper = [string]::Join($newline, @(
        "#if defined(__MINGW32__) && defined(__aarch64__)",
        "int",
        "clock_gettime64 (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)",
        "{",
        "  return check_clock_gettime_fallback (clk_id, ts);",
        "}",
        "#endif"
    ))
    $badPatchedFunctionPrefix = [string]::Join($newline, @(
        "#if defined(__MINGW32__) && defined(__aarch64__) && defined(clock_gettime)",
        "#undef clock_gettime",
        "#endif",
        ""
    ))
    $legacyFunctionStart = [string]::Join($newline, @(
        "int",
        "clock_gettime (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)"
    ))
    $guardedFunctionStart = [string]::Join($newline, @(
        $guard,
        "int",
        "clock_gettime (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)"
    ))
    $badPatchedFunctionStart = [string]::Join($newline, @(
        "static int",
        "check_clock_gettime_fallback (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)"
    ))
    $worsePatchedFunctionStart = [string]::Join($newline, @(
        $badPatchedFunctionPrefix.TrimEnd(),
        "static int",
        "check_clock_gettime_fallback (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)"
    ))

    if (
        -not $clockGettimeSource.Contains($legacyFunctionStart) -and
        -not $clockGettimeSource.Contains($guardedFunctionStart) -and
        -not $clockGettimeSource.Contains($badPatchedFunctionStart) -and
        -not $clockGettimeSource.Contains($worsePatchedFunctionStart)
    ) {
        return
    }

    $patchedClockGettimeSource = $clockGettimeSource
    if (-not $patchedClockGettimeSource.Contains($guardedFunctionStart)) {
        if ($patchedClockGettimeSource.Contains($worsePatchedFunctionStart)) {
            $patchedClockGettimeSource = $patchedClockGettimeSource.Replace($worsePatchedFunctionStart, $guardedFunctionStart)
        } elseif ($patchedClockGettimeSource.Contains($badPatchedFunctionStart)) {
            $patchedClockGettimeSource = $patchedClockGettimeSource.Replace($badPatchedFunctionStart, $guardedFunctionStart)
        } else {
            $patchedClockGettimeSource = $patchedClockGettimeSource.Replace($legacyFunctionStart, $guardedFunctionStart)
        }
    }

    $legacyFunctionEnd = [string]::Join($newline, @(
        "  return 0;",
        "}"
    ))
    $guardedFunctionEnd = [string]::Join($newline, @(
        "  return 0;",
        "}",
        "#endif"
    ))
    $badPatchedFunctionEnd = [string]::Join($newline, @(
        "  return 0;",
        "}",
        "",
        "int",
        "clock_gettime (clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct timespec *ts)",
        "{",
        "  return check_clock_gettime_fallback (clk_id, ts);",
        "}",
        "",
        $clockGettime64Wrapper
    ))
    if (-not $patchedClockGettimeSource.Contains($guardedFunctionEnd)) {
        if ($patchedClockGettimeSource.Contains($badPatchedFunctionEnd)) {
            $patchedClockGettimeSource = $patchedClockGettimeSource.Replace($badPatchedFunctionEnd, $guardedFunctionEnd)
        } else {
            $patchedClockGettimeSource = $patchedClockGettimeSource.Replace($legacyFunctionEnd, $guardedFunctionEnd)
        }
    }
    $clockGettime64WrapperPattern = '(?ms)^#if defined\(__MINGW32__\) && defined\(__aarch64__\)\r?\nint\r?\nclock_gettime64 \(clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct [^\r\n]*\*ts\)\r?\n\{\r?\n  return check_clock_gettime_fallback \(clk_id, ts\);\r?\n\}\r?\n#endif\r?\n?'
    $clockGettime64WrapperBodyPattern = '(?ms)^int\r?\nclock_gettime64 \(clockid_t clk_id CK_ATTRIBUTE_UNUSED, struct [^\r\n]*\*ts\)\r?\n\{\r?\n  return check_clock_gettime_fallback \(clk_id, ts\);\r?\n\}\r?\n?'
    $patchedClockGettimeSource = [regex]::Replace($patchedClockGettimeSource, $clockGettime64WrapperPattern, "")
    $patchedClockGettimeSource = [regex]::Replace($patchedClockGettimeSource, $clockGettime64WrapperBodyPattern, "")

    if ($patchedClockGettimeSource -ne $clockGettimeSource) {
        Set-Content -Path $clockGettimePath -Value $patchedClockGettimeSource -NoNewline
    }
}

function Repair-GstreamerWebRtcTraceEventUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $traceEventPaths = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "webrtc-audio-processing*" -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName "webrtc\rtc_base\trace_event.h" }

        foreach ($traceEventPath in $traceEventPaths) {
            if (-not (Test-Path $traceEventPath)) {
                continue
            }

            $traceEventSource = Get-Content -Raw -Path $traceEventPath
            if ($traceEventSource.Contains("#include <cstdint>")) {
                continue
            }

            $newline = if ($traceEventSource.Contains("`r`n")) { "`r`n" } else { "`n" }
            $inttypesInclude = "#include <inttypes.h>$newline"
            $stringInclude = "#include <string>$newline"
            $patchedTraceEventSource = $null
            if ($traceEventSource.Contains($inttypesInclude)) {
                $patchedTraceEventSource = $traceEventSource.Replace(
                    $inttypesInclude,
                    "#include <inttypes.h>$newline#include <cstdint>$newline"
                )
            } elseif ($traceEventSource.Contains($stringInclude)) {
                $patchedTraceEventSource = $traceEventSource.Replace(
                    $stringInclude,
                    "#include <string>$newline#include <cstdint>$newline"
                )
            } else {
                continue
            }

            if ($patchedTraceEventSource -ne $traceEventSource) {
                Set-Content -Path $traceEventPath -Value $patchedTraceEventSource -NoNewline
            }
        }
    }
}

function Repair-GstreamerWebRtcMultiChannelContentDetectorUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $headerPaths = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "webrtc-audio-processing*" -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName "webrtc\modules\audio_processing\aec3\multi_channel_content_detector.h" }

        foreach ($headerPath in $headerPaths) {
            if (-not (Test-Path $headerPath)) {
                continue
            }

            $headerSource = Get-Content -Raw -Path $headerPath
            if ($headerSource.Contains("#include <cstdint>")) {
                continue
            }

            $newline = if ($headerSource.Contains("`r`n")) { "`r`n" } else { "`n" }
            $vectorInclude = "#include <vector>$newline"
            if (-not $headerSource.Contains($vectorInclude)) {
                continue
            }

            $patchedHeaderSource = $headerSource.Replace(
                $vectorInclude,
                "#include <vector>$newline#include <cstdint>$newline"
            )
            if ($patchedHeaderSource -ne $headerSource) {
                Set-Content -Path $headerPath -Value $patchedHeaderSource -NoNewline
            }
        }
    }
}

function Repair-GstreamerIntrospectionDistutilsUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $giscannerRoots = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "gobject-introspection-*" -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName "giscanner" }

        foreach ($giscannerRoot in $giscannerRoots) {
            $ccompilerPath = Join-Path $giscannerRoot "ccompiler.py"
            if (Test-Path $ccompilerPath) {
                $ccompilerSource = Get-Content -Raw -Path $ccompilerPath
                $patchedCcompilerSource = $ccompilerSource -replace '(?m)^from distutils\.msvccompiler import MSVCCompiler\r?\n', ''
                $patchedCcompilerSource = $patchedCcompilerSource.Replace(
                    "# MSVC9Compiler class, as it does not provide a preprocess()",
                    "# MSVCCompiler class, as it does not provide a preprocess()"
                )
                $patchedCcompilerSource = $patchedCcompilerSource.Replace(
                    'return isinstance(self.compiler, MSVCCompiler)',
                    'return self.compiler.compiler_type == "msvc"'
                )
                $patchedCcompilerSource = $patchedCcompilerSource.Replace(
                    'if isinstance(self.compiler, MSVCCompiler):',
                    'if self.check_is_msvc():'
                )

                if ($patchedCcompilerSource -ne $ccompilerSource) {
                    Set-Content -Path $ccompilerPath -Value $patchedCcompilerSource -NoNewline
                }
            }

            $msvccompilerPath = Join-Path $giscannerRoot "msvccompiler.py"
            if (Test-Path $msvccompilerPath) {
                $msvccompilerSource = Get-Content -Raw -Path $msvccompilerPath
                $newline = if ($msvccompilerSource.Contains("`r`n")) { "`r`n" } else { "`n" }
                $distutilsCompilerDefinition = 'DistutilsMSVCCompiler: Type = type(new_compiler(compiler="msvc"))'
                $patchedMsvccompilerSource = $msvccompilerSource -replace '(?m)^import distutils\r?\n', "from typing import Type$newline"
                $patchedMsvccompilerSource = $patchedMsvccompilerSource.Replace(
                    "from distutils.ccompiler import CCompiler, gen_preprocess_options",
                    "from distutils.ccompiler import CCompiler, gen_preprocess_options, new_compiler"
                )
                if (-not $patchedMsvccompilerSource.Contains($distutilsCompilerDefinition)) {
                    $patchedMsvccompilerSource = $patchedMsvccompilerSource.Replace(
                        "# Implementation, so do our own here.$newline$newline",
                        "# Implementation, so do our own here.$newline$newline$distutilsCompilerDefinition$newline$newline"
                    )
                }
                $patchedMsvccompilerSource = $patchedMsvccompilerSource.Replace(
                    "class MSVCCompiler(distutils.msvccompiler.MSVCCompiler):",
                    "class MSVCCompiler(DistutilsMSVCCompiler):"
                )
                $patchedMsvccompilerSource = $patchedMsvccompilerSource.Replace(
                    "super(distutils.msvccompiler.MSVCCompiler, self).__init__()",
                    "super(DistutilsMSVCCompiler, self).__init__()"
                )
                $patchedMsvccompilerSource = $patchedMsvccompilerSource -replace "(?m)^        if os\.name == 'nt':\r?\n            if isinstance\(self, distutils\.msvc9compiler\.MSVCCompiler\):\r?\n                self\.__version = distutils\.msvc9compiler\.VERSION\r?\n", ''

                if ($patchedMsvccompilerSource -ne $msvccompilerSource) {
                    Set-Content -Path $msvccompilerPath -Value $patchedMsvccompilerSource -NoNewline
                }
            }
        }
    }
}

function Repair-GstreamerPygobjectTrashcanApiUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $pygobjectRoots = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "pygobject-*" -ErrorAction SilentlyContinue

        foreach ($pygobjectRoot in $pygobjectRoots) {
            $resultTuplePath = Join-Path $pygobjectRoot.FullName "gi\pygi-resulttuple.c"
            if (Test-Path $resultTuplePath) {
                $resultTupleSource = Get-Content -Raw -Path $resultTuplePath
                $patchedResultTupleSource = $resultTupleSource.Replace(
                    "    Py_TRASHCAN_SAFE_BEGIN (self)",
                    "    CPy_TRASHCAN_BEGIN (self, resulttuple_dealloc)"
                )
                $patchedResultTupleSource = $patchedResultTupleSource.Replace(
                    "    Py_TRASHCAN_BEGIN (self, resulttuple_dealloc)",
                    "    CPy_TRASHCAN_BEGIN (self, resulttuple_dealloc)"
                )
                $patchedResultTupleSource = $patchedResultTupleSource.Replace(
                    "    Py_TRASHCAN_SAFE_END (self)",
                    "    CPy_TRASHCAN_END (self)"
                )
                $patchedResultTupleSource = $patchedResultTupleSource.Replace(
                    "    Py_TRASHCAN_END",
                    "    CPy_TRASHCAN_END (self)"
                )

                if ($patchedResultTupleSource -ne $resultTupleSource) {
                    Set-Content -Path $resultTuplePath -Value $patchedResultTupleSource -NoNewline
                }
            }

            $utilHeaderPath = Join-Path $pygobjectRoot.FullName "gi\pygi-util.h"
            if (Test-Path $utilHeaderPath) {
                $utilHeaderSource = Get-Content -Raw -Path $utilHeaderPath
                if (-not $utilHeaderSource.Contains("CPy_TRASHCAN_BEGIN(op, dealloc)")) {
                    $newline = if ($utilHeaderSource.Contains("`r`n")) { "`r`n" } else { "`n" }
                    $pySetTypeCompatBlock = [string]::Join($newline, @(
                        "#if PY_VERSION_HEX < 0x030900A4",
                        "#  define Py_SET_TYPE(obj, type) ((Py_TYPE(obj) = (type)), (void)0)",
                        "#endif"
                    ))
                    $trashcanCompatBlock = [string]::Join($newline, @(
                        "#if PY_VERSION_HEX >= 0x03080000",
                        "#  define CPy_TRASHCAN_BEGIN(op, dealloc) Py_TRASHCAN_BEGIN(op, dealloc)",
                        "#  define CPy_TRASHCAN_END(op) Py_TRASHCAN_END",
                        "#else",
                        "#  define CPy_TRASHCAN_BEGIN(op, dealloc) Py_TRASHCAN_SAFE_BEGIN(op)",
                        "#  define CPy_TRASHCAN_END(op) Py_TRASHCAN_SAFE_END(op)",
                        "#endif"
                    ))
                    $patchedUtilHeaderSource = $utilHeaderSource.Replace(
                        $pySetTypeCompatBlock,
                        [string]::Join($newline, @(
                            $pySetTypeCompatBlock,
                            "",
                            $trashcanCompatBlock
                        ))
                    )

                    if ($patchedUtilHeaderSource -ne $utilHeaderSource) {
                        Set-Content -Path $utilHeaderPath -Value $patchedUtilHeaderSource -NoNewline
                    }
                }
            }
        }
    }
}

function Repair-GstreamerPycairoBufferApiUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $surfacePaths = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "pycairo*" -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName "cairo\surface.c" }

        foreach ($surfacePath in $surfacePaths) {
            if (-not (Test-Path $surfacePath)) {
                continue
            }

            $surfaceSource = Get-Content -Raw -Path $surfacePath
            $newline = if ($surfaceSource.Contains("`r`n")) { "`r`n" } else { "`n" }
            $patchedSurfaceSource = $surfaceSource
            $regexOptions = [System.Text.RegularExpressions.RegexOptions]::Singleline -bor
                [System.Text.RegularExpressions.RegexOptions]::Multiline -bor
                [System.Text.RegularExpressions.RegexOptions]::CultureInvariant
            $replacePycairoBlock = {
                param(
                    [Parameter(Mandatory = $true)][string]$Source,
                    [Parameter(Mandatory = $true)][string]$Pattern,
                    [Parameter(Mandatory = $true)][string]$Replacement
                )

                $regex = [regex]::new($Pattern, $regexOptions)
                if (-not $regex.IsMatch($Source)) {
                    return $Source
                }

                return $regex.Replace(
                    $Source,
                    [System.Text.RegularExpressions.MatchEvaluator]{
                        param($match)
                        $Replacement
                    },
                    1
                )
            }

            $surfaceBufferViewKeyDefinition = [string]::Join($newline, @(
                "static const cairo_user_data_key_t surface_is_mapped_image;",
                "static const cairo_user_data_key_t surface_buffer_view_key;"
            ))
            if (-not $patchedSurfaceSource.Contains("static const cairo_user_data_key_t surface_buffer_view_key;")) {
                $patchedSurfaceSource = & $replacePycairoBlock `
                    $patchedSurfaceSource `
                    '(?m)^static const cairo_user_data_key_t surface_is_mapped_image;\r?$' `
                    $surfaceBufferViewKeyDefinition
            }

            $bufferAwareSurfaceFinish = [string]::Join($newline, @(
                "static PyObject *",
                "surface_finish (PycairoSurface *o, PyObject *ignored) {",
                "  cairo_surface_finish (o->surface);",
                "  Py_CLEAR(o->base);",
                "",
                "  /* After an image surface is finished it won't access the buffer and",
                "  we can release it */",
                "  cairo_surface_set_user_data(",
                "    o->surface, &surface_buffer_view_key, NULL, NULL);",
                "",
                "  RETURN_NULL_IF_CAIRO_SURFACE_ERROR(o->surface);",
                "  Py_RETURN_NONE;",
                "}"
            ))
            $patchedSurfaceSource = & $replacePycairoBlock `
                $patchedSurfaceSource `
                '(?ms)^static PyObject \*\r?\nsurface_finish \(PycairoSurface \*o, PyObject \*ignored\) \{.*?^\}' `
                $bufferAwareSurfaceFinish

            $bufferAwareMimeBlock = [string]::Join($newline, @(
                "static void",
                "_destroy_mime_data_func (PyObject *user_data) {",
                "  cairo_surface_t *surface;",
                "  Py_buffer *view;",
                "  PyObject *mime_intern;",
                "",
                "  PyGILState_STATE gstate = PyGILState_Ensure();",
                "",
                "  /* Remove the user data holding the source object */",
                "  surface = PyCapsule_GetPointer(PyTuple_GET_ITEM(user_data, 0), NULL);",
                "  view = PyCapsule_GetPointer(PyTuple_GET_ITEM(user_data, 1), NULL);",
                "  mime_intern = PyTuple_GET_ITEM(user_data, 3);",
                "  cairo_surface_set_user_data(",
                "    surface, (cairo_user_data_key_t *)mime_intern, NULL, NULL);",
                "",
                "  /* Destroy the user data */",
                "  PyBuffer_Release (view);",
                "  PyMem_Free (view);",
                "  Py_DECREF(user_data);",
                "",
                "  PyGILState_Release(gstate);",
                "}",
                "",
                "static PyObject *",
                "surface_set_mime_data (PycairoSurface *o, PyObject *args) {",
                "  PyObject *obj, *user_data, *mime_intern, *surface_capsule, *view_capsule;",
                "  Py_buffer *view;",
                "  const char *mime_type;",
                "  int res;",
                "  cairo_status_t status;",
                "",
                "  if (!PyArg_ParseTuple(args, ""sO:Surface.set_mime_data"", &mime_type, &obj))",
                "    return NULL;",
                "",
                "  if (obj == Py_None) {",
                "    status = cairo_surface_set_mime_data (",
                "      o->surface, mime_type, NULL, 0, NULL, NULL);",
                "",
                "    RETURN_NULL_IF_CAIRO_ERROR(status);",
                "    Py_RETURN_NONE;",
                "  }",
                "",
                "  view = PyMem_Malloc (sizeof (Py_buffer));",
                "  if (view == NULL) {",
                "    PyErr_NoMemory ();",
                "    return NULL;",
                "  }",
                "",
                "  res = PyObject_GetBuffer (obj, view, PyBUF_SIMPLE);",
                "  if (res == -1) {",
                "    PyMem_Free (view);",
                "    return NULL;",
                "  }",
                "",
                "  /* We use the interned mime type string as user data key and store the",
                "   * passed in object with it. This allows us to return the same object in",
                "   * surface_get_mime_data().",
                "   */",
                "  mime_intern = PYCAIRO_PyUnicode_InternFromString(mime_type);",
                "  surface_capsule = PyCapsule_New(o->surface, NULL, NULL);",
                "  view_capsule = PyCapsule_New(view, NULL, NULL);",
                "  user_data = Py_BuildValue(""(NNOO)"", surface_capsule, view_capsule, obj, mime_intern);",
                "  if (user_data == NULL) {",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    return NULL;",
                "  }",
                "",
                "  status = cairo_surface_set_user_data(",
                "    o->surface, (cairo_user_data_key_t *)mime_intern, user_data,",
                "    (cairo_destroy_func_t)_destroy_mime_user_data_func);",
                "  if (status != CAIRO_STATUS_SUCCESS) {",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    Py_DECREF(user_data);",
                "    Pycairo_Check_Status (status);",
                "    return NULL;",
                "  }",
                "",
                "  Py_INCREF(user_data);",
                "  status = cairo_surface_set_mime_data (",
                "    o->surface, mime_type, view->buf, (unsigned long)view->len,",
                "    (cairo_destroy_func_t)_destroy_mime_data_func, user_data);",
                "  if (status != CAIRO_STATUS_SUCCESS) {",
                "    cairo_surface_set_user_data(",
                "      o->surface, (cairo_user_data_key_t *)mime_intern, NULL, NULL);",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    Py_DECREF(user_data);",
                "    Pycairo_Check_Status (status);",
                "    return NULL;",
                "  }",
                "",
                "  Py_RETURN_NONE;",
                "}",
                "",
                "static PyObject *",
                "surface_get_mime_data (PycairoSurface *o, PyObject *args) {",
                "  PyObject *user_data, *obj, *mime_intern;",
                "  const char *mime_type;",
                "  const unsigned char *buffer;",
                "  unsigned long buffer_len;",
                "",
                "  if (!PyArg_ParseTuple(args, ""s:Surface.get_mime_data"", &mime_type))",
                "    return NULL;",
                "",
                "  cairo_surface_get_mime_data (o->surface, mime_type, &buffer, &buffer_len);",
                "  if (buffer == NULL) {",
                "    Py_RETURN_NONE;",
                "  }",
                "",
                "  mime_intern = PYCAIRO_PyUnicode_InternFromString(mime_type);",
                "  user_data = cairo_surface_get_user_data(",
                "    o->surface, (cairo_user_data_key_t *)mime_intern);",
                "",
                "  if (user_data == NULL) {",
                "    /* In case the mime data wasn't set through the Python API just copy it */",
                "    return Py_BuildValue(PYCAIRO_DATA_FORMAT ""#"", buffer, buffer_len);",
                "  } else {",
                "    obj = PyTuple_GET_ITEM(user_data, 2);",
                "    Py_INCREF(obj);",
                "    return obj;",
                "  }",
                "}"
            ))
            $patchedSurfaceSource = & $replacePycairoBlock `
                $patchedSurfaceSource `
                '(?ms)^static void\r?\n_destroy_mime_data_func \(PyObject \*user_data\) \{.*?^\}\r?\n\r?\nstatic PyObject \*\r?\nsurface_set_mime_data \(PycairoSurface \*o, PyObject \*args\) \{.*?^\}\r?\n\r?\nstatic PyObject \*\r?\nsurface_get_mime_data \(PycairoSurface \*o, PyObject \*args\) \{.*?^\}' `
                $bufferAwareMimeBlock

            $bufferAwareImageSurfaceBlock = [string]::Join($newline, @(
                "static void",
                "_release_buffer_destroy_func(void *user_data) {",
                "  Py_buffer *view = (Py_buffer *)user_data;",
                "  PyGILState_STATE gstate = PyGILState_Ensure();",
                "  PyBuffer_Release (view);",
                "  PyMem_Free (view);",
                "  PyGILState_Release(gstate);",
                "}",
                "",
                "/* METH_CLASS */",
                "static PyObject *",
                "image_surface_create_for_data (PyTypeObject *type, PyObject *args) {",
                "  cairo_surface_t *surface;",
                "  cairo_format_t format;",
                "  unsigned char *buffer;",
                "  int width, height, stride = -1, res, format_arg;",
                "  Py_buffer *view;",
                "  PyObject *obj;",
                "  cairo_status_t status;",
                "",
                "  if (!PyArg_ParseTuple (args, ""Oiii|i:ImageSurface.create_for_data"",",
                "                         &obj, &format_arg, &width, &height, &stride))",
                "    return NULL;",
                "",
                "  format = (cairo_format_t)format_arg;",
                "",
                "  view = PyMem_Malloc (sizeof (Py_buffer));",
                "  if (view == NULL) {",
                "    PyErr_NoMemory ();",
                "    return NULL;",
                "  }",
                "",
                "  res = PyObject_GetBuffer (obj, view, PyBUF_WRITABLE);",
                "  if (res == -1) {",
                "    PyMem_Free (view);",
                "    return NULL;",
                "  }",
                "  buffer = (unsigned char *)view->buf;",
                "",
                "  if (width <= 0) {",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    PyErr_SetString(PyExc_ValueError, ""width must be positive"");",
                "    return NULL;",
                "  }",
                "  if (height <= 0) {",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    PyErr_SetString(PyExc_ValueError, ""height must be positive"");",
                "    return NULL;",
                "  }",
                "  /* if stride is missing, calculate it from width */",
                "  if (stride < 0) {",
                "    stride = cairo_format_stride_for_width (format, width);",
                "    if (stride == -1){",
                "      PyBuffer_Release (view);",
                "      PyMem_Free (view);",
                "      PyErr_SetString(PyExc_ValueError,",
                "		      ""format is invalid or the width too large"");",
                "      return NULL;",
                "    }",
                "  }",
                "  if (height * stride > view->len) {",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    PyErr_SetString(PyExc_TypeError, ""buffer is not long enough"");",
                "    return NULL;",
                "  }",
                "  Py_BEGIN_ALLOW_THREADS;",
                "  surface = cairo_image_surface_create_for_data (buffer, format, width,",
                "						 height, stride);",
                "  Py_END_ALLOW_THREADS;",
                "",
                "  status = cairo_surface_set_user_data(",
                "    surface, &surface_buffer_view_key, view,",
                "    (cairo_destroy_func_t)_release_buffer_destroy_func);",
                "  if (Pycairo_Check_Status (status)) {",
                "    cairo_surface_destroy (surface);",
                "    PyBuffer_Release (view);",
                "    PyMem_Free (view);",
                "    return NULL;",
                "  }",
                "",
                "  return _surface_create_with_object(surface, obj);",
                "}"
            ))
            $patchedSurfaceSource = & $replacePycairoBlock `
                $patchedSurfaceSource `
                '(?ms)^(?:static void\r?\n_release_buffer_destroy_func\(void \*user_data\) \{.*?^\}\r?\n\r?\n)?/\* METH_CLASS \*/\r?\nstatic PyObject \*\r?\nimage_surface_create_for_data \(PyTypeObject \*type, PyObject \*args\) \{.*?^\}' `
                $bufferAwareImageSurfaceBlock

            $guardedXpybBufferAccess = [string]::Join($newline, @(
                "const void *",
                "xpyb2struct(PyObject *obj, Py_ssize_t *len)",
                "{",
                "    const void *data;",
                "",
                "#if PY_MAJOR_VERSION < 3",
                "    if (PyObject_AsReadBuffer(obj, &data, len) < 0)",
                "        return NULL;",
                "",
                "    return data;",
                "#endif",
                "",
                "  // buffer function disabled",
                "  return NULL;",
                "}"
            ))
            $patchedSurfaceSource = & $replacePycairoBlock `
                $patchedSurfaceSource `
                '(?ms)^const void \*\r?\nxpyb2struct\(PyObject \*obj, Py_ssize_t \*len\)\r?\n\{.*?^\}' `
                $guardedXpybBufferAccess

            if ($patchedSurfaceSource -ne $surfaceSource) {
                Set-Content -Path $surfacePath -Value $patchedSurfaceSource -NoNewline
            }
        }
    }
}

function Repair-GstreamerAbseilTimeZoneLookupUsage {
    param(
        [Parameter(Mandatory = $true)][string]$GstreamerRoot,
        [string]$BuildRoot
    )

    $candidateRoots = [System.Collections.Generic.List[string]]::new()
    foreach ($candidateRoot in @($GstreamerRoot, $BuildRoot)) {
        if (-not $candidateRoot -or $candidateRoots.Contains($candidateRoot) -or -not (Test-Path $candidateRoot)) {
            continue
        }

        $candidateRoots.Add($candidateRoot)
    }

    foreach ($candidateRoot in $candidateRoots) {
        $subprojectsRoot = Join-Path $candidateRoot "subprojects"
        if (-not (Test-Path $subprojectsRoot)) {
            continue
        }

        $timeZoneLookupPaths = Get-ChildItem -Path $subprojectsRoot -Directory -Filter "abseil-cpp-*" -ErrorAction SilentlyContinue |
            ForEach-Object { Join-Path $_.FullName "absl\time\internal\cctz\src\time_zone_lookup.cc" }

        foreach ($timeZoneLookupPath in $timeZoneLookupPaths) {
            if (-not (Test-Path $timeZoneLookupPath)) {
                continue
            }

            $timeZoneLookupSource = Get-Content -Raw -Path $timeZoneLookupPath
            if ($timeZoneLookupSource.Contains("ucal_getTimeZoneIDForWindowsID")) {
                continue
            }

            $newline = if ($timeZoneLookupSource.Contains("`r`n")) { "`r`n" } else { "`n" }
            $patchedTimeZoneLookupSource = $timeZoneLookupSource.Replace(
                [string]::Join($newline, @(
                    "#if defined(_WIN32)",
                    "#include <sdkddkver.h>",
                    "// Include only when the SDK is for Windows 10 (and later), and the binary is",
                    "// targeted for Windows XP and later.",
                    "// Note: The Windows SDK added windows.globalization.h file for Windows 10, but",
                    "// MinGW did not add it until NTDDI_WIN10_NI (SDK version 10.0.22621.0).",
                    "#if ((defined(_WIN32_WINNT_WIN10) && !defined(__MINGW32__)) || \\",
                    "     (defined(NTDDI_WIN10_NI) && NTDDI_VERSION >= NTDDI_WIN10_NI)) && \\",
                    "    (_WIN32_WINNT >= _WIN32_WINNT_WINXP)",
                    "#define USE_WIN32_LOCAL_TIME_ZONE",
                    "#include <roapi.h>",
                    "#include <tchar.h>",
                    "#include <wchar.h>",
                    "#include <windows.globalization.h>",
                    "#include <windows.h>",
                    "#include <winstring.h>",
                    "#endif",
                    "#endif"
                )),
                [string]::Join($newline, @(
                    "#if defined(_WIN32)",
                    "// Load the Windows ICU entry point dynamically instead of depending on",
                    "// whichever <icu.h> happens to appear first on the MinGW include path.",
                    "#define USE_WIN32_LOCAL_TIME_ZONE",
                    "#include <windows.h>",
                    "#include <timezoneapi.h>",
                    "",
                    "#include <atomic>",
                    "#endif  // _WIN32"
                ))
            )
            $patchedTimeZoneLookupSource = $patchedTimeZoneLookupSource.Replace(
                [string]::Join($newline, @(
                    "#include <cstdlib>",
                    "#include <cstring>",
                    "#include <string>"
                )),
                [string]::Join($newline, @(
                    "#include <array>",
                    "#include <cstdint>",
                    "#include <cstdlib>",
                    "#include <cstring>",
                    "#include <string>"
                ))
            )
            $patchedTimeZoneLookupSource = $patchedTimeZoneLookupSource.Replace(
                [string]::Join($newline, @(
                    "// Calls the WinRT Calendar.GetTimeZone method to obtain the IANA ID of the",
                    "// local time zone. Returns an empty vector in case of an error.",
                    "std::string win32_local_time_zone(const HMODULE combase) {",
                    "  std::string result;",
                    "  const auto ro_activate_instance =",
                    "      reinterpret_cast<decltype(&RoActivateInstance)>(",
                    '          GetProcAddress(combase, "RoActivateInstance"));',
                    "  if (!ro_activate_instance) {",
                    "    return result;",
                    "  }",
                    "  const auto windows_create_string_reference =",
                    "      reinterpret_cast<decltype(&WindowsCreateStringReference)>(",
                    '          GetProcAddress(combase, "WindowsCreateStringReference"));',
                    "  if (!windows_create_string_reference) {",
                    "    return result;",
                    "  }",
                    "  const auto windows_delete_string =",
                    "      reinterpret_cast<decltype(&WindowsDeleteString)>(",
                    '          GetProcAddress(combase, "WindowsDeleteString"));',
                    "  if (!windows_delete_string) {",
                    "    return result;",
                    "  }",
                    "  const auto windows_get_string_raw_buffer =",
                    "      reinterpret_cast<decltype(&WindowsGetStringRawBuffer)>(",
                    '          GetProcAddress(combase, "WindowsGetStringRawBuffer"));',
                    "  if (!windows_get_string_raw_buffer) {",
                    "    return result;",
                    "  }",
                    "",
                    "  // The string returned by WindowsCreateStringReference doesn't need to be",
                    "  // deleted.",
                    "  HSTRING calendar_class_id;",
                    "  HSTRING_HEADER calendar_class_id_header;",
                    "  HRESULT hr = windows_create_string_reference(",
                    "      RuntimeClass_Windows_Globalization_Calendar,",
                    "      sizeof(RuntimeClass_Windows_Globalization_Calendar) / sizeof(wchar_t) - 1,",
                    "      &calendar_class_id_header, &calendar_class_id);",
                    "  if (FAILED(hr)) {",
                    "    return result;",
                    "  }",
                    "",
                    "  IInspectable* calendar;",
                    "  hr = ro_activate_instance(calendar_class_id, &calendar);",
                    "  if (FAILED(hr)) {",
                    "    return result;",
                    "  }",
                    "",
                    "  ABI::Windows::Globalization::ITimeZoneOnCalendar* time_zone;",
                    "  hr = calendar->QueryInterface(IID_PPV_ARGS(&time_zone));",
                    "  if (FAILED(hr)) {",
                    "    calendar->Release();",
                    "    return result;",
                    "  }",
                    "",
                    "  HSTRING tz_hstr;",
                    "  hr = time_zone->GetTimeZone(&tz_hstr);",
                    "  if (SUCCEEDED(hr)) {",
                    "    UINT32 wlen;",
                    "    const PCWSTR tz_wstr = windows_get_string_raw_buffer(tz_hstr, &wlen);",
                    "    if (tz_wstr) {",
                    "      const int size =",
                    "          WideCharToMultiByte(CP_UTF8, 0, tz_wstr, static_cast<int>(wlen),",
                    "                              nullptr, 0, nullptr, nullptr);",
                    "      result.resize(static_cast<size_t>(size));",
                    "      WideCharToMultiByte(CP_UTF8, 0, tz_wstr, static_cast<int>(wlen),",
                    "                          &result[0], size, nullptr, nullptr);",
                    "    }",
                    "    windows_delete_string(tz_hstr);",
                    "  }",
                    "  time_zone->Release();",
                    "  calendar->Release();",
                    "  return result;",
                    "}"
                )),
                [string]::Join($newline, @(
                    "// MSYS2's MinGW headers do not reliably expose the WinRT string helpers that",
                    "// this older Abseil release expects, and some toolchains resolve <icu.h> to",
                    "// a header that does not declare the Windows ICU API we need here.",
                    "using UcalGetTimeZoneIDForWindowsIDFn = int32_t(WINAPI*)(",
                    "    const wchar_t*, int32_t, const char*, wchar_t*, int32_t, int*);",
                    "",
                    "// True if we have already failed to load the API.",
                    "static std::atomic_bool g_ucal_getTimeZoneIDForWindowsIDUnavailable;",
                    "static std::atomic<UcalGetTimeZoneIDForWindowsIDFn>",
                    "    g_ucal_getTimeZoneIDForWindowsIDRef;",
                    "",
                    "std::string win32_local_time_zone() {",
                    "  // If we have already failed to load the API, then just give up.",
                    "  if (g_ucal_getTimeZoneIDForWindowsIDUnavailable.load()) {",
                    '    return "";',
                    "  }",
                    "",
                    "  auto ucal_getTimeZoneIDForWindowsIDFunc =",
                    "      g_ucal_getTimeZoneIDForWindowsIDRef.load();",
                    "  if (ucal_getTimeZoneIDForWindowsIDFunc == nullptr) {",
                    "    // If we have already failed to load the API, then just give up.",
                    "    if (g_ucal_getTimeZoneIDForWindowsIDUnavailable.load()) {",
                    '      return "";',
                    "    }",
                    "",
                    "    const HMODULE icudll =",
                    '        ::LoadLibraryExW(L"icu.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);',
                    "",
                    "    if (icudll == nullptr) {",
                    "      g_ucal_getTimeZoneIDForWindowsIDUnavailable.store(true);",
                    '      return "";',
                    "    }",
                    "",
                    "    ucal_getTimeZoneIDForWindowsIDFunc =",
                    "        reinterpret_cast<UcalGetTimeZoneIDForWindowsIDFn>(",
                    '            ::GetProcAddress(icudll, "ucal_getTimeZoneIDForWindowsID"));',
                    "",
                    "    if (ucal_getTimeZoneIDForWindowsIDFunc == nullptr) {",
                    "      g_ucal_getTimeZoneIDForWindowsIDUnavailable.store(true);",
                    '      return "";',
                    "    }",
                    "    // store-race is not a problem here, because ::GetProcAddress() returns the",
                    "    // same address for the same function in the same DLL.",
                    "    g_ucal_getTimeZoneIDForWindowsIDRef.store(",
                    "        ucal_getTimeZoneIDForWindowsIDFunc);",
                    "",
                    "    // We intentionally do not call ::FreeLibrary() here to avoid frequent DLL",
                    '    // loadings and unloading. As "icu.dll" is a system library, keeping it on',
                    "    // memory is supposed to have no major drawback.",
                    "  }",
                    "",
                    "  DYNAMIC_TIME_ZONE_INFORMATION info = {};",
                    "  if (::GetDynamicTimeZoneInformation(&info) == TIME_ZONE_ID_INVALID) {",
                    '    return "";',
                    "  }",
                    "",
                    "  std::array<wchar_t, 128> buffer;",
                    "  int status = 0;",
                    "  const auto num_chars_in_buffer = ucal_getTimeZoneIDForWindowsIDFunc(",
                    "      info.TimeZoneKeyName, -1, nullptr,",
                    "      buffer.data(), static_cast<int32_t>(buffer.size()), &status);",
                    "  if (status != 0 || num_chars_in_buffer <= 0 ||",
                    "      num_chars_in_buffer > static_cast<int32_t>(buffer.size())) {",
                    '    return "";',
                    "  }",
                    "",
                    "  const int num_bytes_in_utf8 = ::WideCharToMultiByte(",
                    "      CP_UTF8, 0, buffer.data(),",
                    "      static_cast<int>(num_chars_in_buffer), nullptr, 0, nullptr, nullptr);",
                    "  std::string local_time_str;",
                    "  local_time_str.resize(static_cast<size_t>(num_bytes_in_utf8));",
                    "  ::WideCharToMultiByte(",
                    "      CP_UTF8, 0, buffer.data(),",
                    "      static_cast<int>(num_chars_in_buffer), &local_time_str[0],",
                    "      num_bytes_in_utf8, nullptr, nullptr);",
                    "  return local_time_str;",
                    "}"
                ))
            )
            $patchedTimeZoneLookupSource = $patchedTimeZoneLookupSource.Replace(
                [string]::Join($newline, @(
                    "  // Use the WinRT Calendar class to get the local time zone. This feature is",
                    "  // available on Windows 10 and later. The library is dynamically linked to",
                    "  // maintain binary compatibility with Windows XP - Windows 7. On Windows 8,",
                    "  // The combase.dll API functions are available but the RoActivateInstance",
                    "  // call will fail for the Calendar class.",
                    "  std::string winrt_tz;",
                    "  const HMODULE combase =",
                    '      LoadLibraryEx(_T("combase.dll"), nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);',
                    "  if (combase) {",
                    "    const auto ro_initialize = reinterpret_cast<decltype(&::RoInitialize)>(",
                    '        GetProcAddress(combase, "RoInitialize"));',
                    "    const auto ro_uninitialize = reinterpret_cast<decltype(&::RoUninitialize)>(",
                    '        GetProcAddress(combase, "RoUninitialize"));',
                    "    if (ro_initialize && ro_uninitialize) {",
                    "      const HRESULT hr = ro_initialize(RO_INIT_MULTITHREADED);",
                    "      // RPC_E_CHANGED_MODE means that a previous RoInitialize call specified",
                    "      // a different concurrency model. The WinRT runtime is initialized and",
                    "      // should work for our purpose here, but we should *not* call",
                    "      // RoUninitialize because it's a failure.",
                    "      if (SUCCEEDED(hr) || hr == RPC_E_CHANGED_MODE) {",
                    "        winrt_tz = win32_local_time_zone(combase);",
                    "        if (SUCCEEDED(hr)) {",
                    "          ro_uninitialize();",
                    "        }",
                    "      }",
                    "    }",
                    "    FreeLibrary(combase);",
                    "  }",
                    "  if (!winrt_tz.empty()) {",
                    "    zone = winrt_tz.c_str();"
                )),
                [string]::Join($newline, @(
                    "  std::string win32_tz = win32_local_time_zone();",
                    "  if (!win32_tz.empty()) {",
                    "    zone = win32_tz.c_str();"
                ))
            )

            if ($patchedTimeZoneLookupSource -ne $timeZoneLookupSource) {
                Set-Content -Path $timeZoneLookupPath -Value $patchedTimeZoneLookupSource -NoNewline
            }
        }
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
$UxPlayRefFile = Join-Path $WorkRoot "uxplay.ref"
$GstRoot = Join-Path $WorkRoot "gst-root"
$GstBuild = Join-Path $WorkRoot "gstreamer-build"
$GstSrc = Join-Path $WorkRoot "gstreamer"
$GstreamerRefFile = Join-Path $WorkRoot "gstreamer.ref"

New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

if ($GstreamerSource -ne "source") {
    throw "unsupported Windows GStreamerSource=$GstreamerSource"
}

$DnssdSupportPath = Build-DnssdStubLibrary -Target $Target -WorkRoot $WorkRoot

if (Stage-CachedRuntimeIfAvailable -OutDir $OutDir -BeaconHelperRelpath $BeaconHelperRelpath -Target $Target -UxPlaySrc $UxPlaySrc -UxPlayBuild $UxPlayBuild -GstRoot $GstRoot -DnssdSupportPath $DnssdSupportPath) {
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
Repair-UxPlayDnsSdHeaderUsage -UxPlayRoot $UxPlaySrc
Repair-GstreamerLibffiFfsUsage -GstreamerRoot $GstSrc -Target $Target
Repair-GstreamerFilesinkFtruncateUsage -GstreamerRoot $GstSrc
Repair-GstreamerD3D11WinApiAppHeaderUsage -GstreamerRoot $GstSrc
Repair-GstreamerD3D11WinRTCaptureNamespaceUsage -GstreamerRoot $GstSrc
Repair-GstreamerD3D12WgcProbeUsage -GstreamerRoot $GstSrc
Repair-GstreamerD3D12GraphicsCaptureHeaderUsage -GstreamerRoot $GstSrc
Repair-GstreamerGstCheckThreadDependencyUsage -GstreamerRoot $GstSrc -Target $Target
Repair-GstreamerLibcheckClockGettimeUsage -GstreamerRoot $GstSrc -Target $Target
Repair-GstreamerWebRtcTraceEventUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
Repair-GstreamerWebRtcMultiChannelContentDetectorUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
Repair-GstreamerIntrospectionDistutilsUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
Repair-GstreamerPygobjectTrashcanApiUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
Repair-GstreamerPycairoBufferApiUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
Repair-GstreamerAbseilTimeZoneLookupUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild

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
# Opus only implements Windows ARM RTCD via _MSC_VER, but the MSYS2 clangarm64
# toolchain uses GNU-style compilation and trips armcpu.c's unsupported path.
if ($Target -eq "aarch64-pc-windows-msvc") {
    $mesonSetupArgs += "-Dopus:rtcd=disabled"
}
if (-not (Test-GstreamerInstallReady -InstallRoot $GstRoot)) {
    foreach ($resetPath in @($GstBuild, $GstRoot)) {
        if (Test-Path $resetPath) {
            Remove-Item $resetPath -Recurse -Force
        }
    }
    New-Item -ItemType Directory -Force -Path $GstRoot | Out-Null
    & $mesonInvocation.Command @mesonSetupArgs
    Repair-GstreamerLibffiFfsUsage -GstreamerRoot $GstSrc -Target $Target
    Repair-GstreamerFilesinkFtruncateUsage -GstreamerRoot $GstSrc
    Repair-GstreamerD3D11WinApiAppHeaderUsage -GstreamerRoot $GstSrc
    Repair-GstreamerD3D11WinRTCaptureNamespaceUsage -GstreamerRoot $GstSrc
    Repair-GstreamerD3D12WgcProbeUsage -GstreamerRoot $GstSrc
    Repair-GstreamerD3D12GraphicsCaptureHeaderUsage -GstreamerRoot $GstSrc
    Repair-GstreamerGstCheckThreadDependencyUsage -GstreamerRoot $GstSrc -Target $Target
    Repair-GstreamerLibcheckClockGettimeUsage -GstreamerRoot $GstSrc -Target $Target
    Repair-GstreamerWebRtcTraceEventUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
    Repair-GstreamerWebRtcMultiChannelContentDetectorUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
    Repair-GstreamerIntrospectionDistutilsUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
    Repair-GstreamerPygobjectTrashcanApiUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
    Repair-GstreamerPycairoBufferApiUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
    Repair-GstreamerAbseilTimeZoneLookupUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
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
$UxPlaySupportPaths = [System.Collections.Generic.List[string]]::new()
foreach ($supportPath in Resolve-UxPlaySupportPaths `
  -UxPlayExecutable $UxPlayExecutable `
  -GstreamerRoot $GstRoot `
  -Target $Target) {
    if (-not $UxPlaySupportPaths.Contains($supportPath)) {
        $UxPlaySupportPaths.Add($supportPath)
    }
}
foreach ($supportPath in Resolve-AdditionalRuntimeSupportPaths -Target $Target) {
    if (-not $UxPlaySupportPaths.Contains($supportPath)) {
        $UxPlaySupportPaths.Add($supportPath)
    }
}
if (-not $UxPlaySupportPaths.Contains($DnssdSupportPath)) {
    $UxPlaySupportPaths.Add($DnssdSupportPath)
}

Invoke-PrepareDirectRuntime `
  -Target $Target `
  -OutDir $OutDir `
  -UxPlayExecutable $UxPlayExecutable `
  -UxPlaySupportPaths $UxPlaySupportPaths `
  -GstreamerRoot $GstRoot `
  -BeaconScript (Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py") `
  -BeaconHelperRelpath $BeaconHelperRelpath
