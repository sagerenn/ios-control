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
                    "// Include only when <icu.h> is available.",
                    "// https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-",
                    "// https://devblogs.microsoft.com/oldnewthing/20210527-00/?p=105255",
                    "#if defined(__has_include)",
                    "#if __has_include(<icu.h>)",
                    "#define USE_WIN32_LOCAL_TIME_ZONE",
                    "#include <windows.h>",
                    '#pragma push_macro("_WIN32_WINNT")',
                    '#pragma push_macro("NTDDI_VERSION")',
                    "// Minimum _WIN32_WINNT and NTDDI_VERSION to use ucal_getTimeZoneIDForWindowsID",
                    "#undef _WIN32_WINNT",
                    "#define _WIN32_WINNT 0x0A00  // == _WIN32_WINNT_WIN10",
                    "#undef NTDDI_VERSION",
                    "#define NTDDI_VERSION 0x0A000004  // == NTDDI_WIN10_RS3",
                    "#include <icu.h>",
                    '#pragma pop_macro("NTDDI_VERSION")',
                    '#pragma pop_macro("_WIN32_WINNT")',
                    "#include <timezoneapi.h>",
                    "",
                    "#include <atomic>",
                    "#endif  // __has_include(<icu.h>)",
                    "#endif  // __has_include",
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
                    "// this older Abseil release expects. Update the vendored file to the newer",
                    "// ICU-based upstream implementation before compiling GStreamer on Windows.",
                    "// True if we have already failed to load the API.",
                    "static std::atomic_bool g_ucal_getTimeZoneIDForWindowsIDUnavailable;",
                    "static std::atomic<decltype(ucal_getTimeZoneIDForWindowsID)*>",
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
                    "        reinterpret_cast<decltype(ucal_getTimeZoneIDForWindowsID)*>(",
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
                    "  std::array<UChar, 128> buffer;",
                    "  UErrorCode status = U_ZERO_ERROR;",
                    "  const auto num_chars_in_buffer = ucal_getTimeZoneIDForWindowsIDFunc(",
                    "      reinterpret_cast<const UChar*>(info.TimeZoneKeyName), -1, nullptr,",
                    "      buffer.data(), static_cast<int32_t>(buffer.size()), &status);",
                    "  if (status != U_ZERO_ERROR || num_chars_in_buffer <= 0 ||",
                    "      num_chars_in_buffer > static_cast<int32_t>(buffer.size())) {",
                    '    return "";',
                    "  }",
                    "",
                    "  const int num_bytes_in_utf8 = ::WideCharToMultiByte(",
                    "      CP_UTF8, 0, reinterpret_cast<const wchar_t*>(buffer.data()),",
                    "      static_cast<int>(num_chars_in_buffer), nullptr, 0, nullptr, nullptr);",
                    "  std::string local_time_str;",
                    "  local_time_str.resize(static_cast<size_t>(num_bytes_in_utf8));",
                    "  ::WideCharToMultiByte(",
                    "      CP_UTF8, 0, reinterpret_cast<const wchar_t*>(buffer.data()),",
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
Repair-GstreamerIntrospectionDistutilsUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
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
if (-not (Test-GstreamerInstallReady -InstallRoot $GstRoot)) {
    foreach ($resetPath in @($GstBuild, $GstRoot)) {
        if (Test-Path $resetPath) {
            Remove-Item $resetPath -Recurse -Force
        }
    }
    New-Item -ItemType Directory -Force -Path $GstRoot | Out-Null
    & $mesonInvocation.Command @mesonSetupArgs
    Repair-GstreamerLibffiFfsUsage -GstreamerRoot $GstSrc -Target $Target
    Repair-GstreamerIntrospectionDistutilsUsage -GstreamerRoot $GstSrc -BuildRoot $GstBuild
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

Invoke-PrepareDirectRuntime `
  -Target $Target `
  -OutDir $OutDir `
  -UxPlayExecutable $UxPlayExecutable `
  -GstreamerRoot $GstRoot `
  -BeaconScript (Join-Path $UxPlaySrc "Bluetooth_LE_beacon\uxplay-beacon.py") `
  -BeaconHelperRelpath $BeaconHelperRelpath
