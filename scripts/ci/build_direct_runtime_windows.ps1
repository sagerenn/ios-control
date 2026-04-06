param(
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$GstreamerSource,
    [Parameter(Mandatory = $true)][string]$OutDir,
    [Parameter(Mandatory = $true)][string]$BeaconHelperRelpath
)

$ErrorActionPreference = "Stop"

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

if (Test-Path $WorkRoot) {
    Remove-Item $WorkRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

git clone --depth 1 --branch $env:UXPLAY_REF https://github.com/FDH2/UxPlay.git $UxPlaySrc
cmake -S $UxPlaySrc -B $UxPlayBuild -DCMAKE_BUILD_TYPE=Release
cmake --build $UxPlayBuild --config Release --parallel

if ($GstreamerSource -eq "download") {
    $zipPath = Join-Path $WorkRoot "gstreamer.zip"
    $uri = "https://gstreamer.freedesktop.org/data/pkg/windows/$($env:GSTREAMER_VERSION)/msvc/gstreamer-1.0-msvc-x86_64-$($env:GSTREAMER_VERSION)-merge-modules.zip"
    Invoke-WebRequest -Uri $uri -OutFile $zipPath
    Expand-Archive -Path $zipPath -DestinationPath $GstRoot -Force
} elseif ($GstreamerSource -eq "source") {
    git clone --depth 1 --branch $env:GSTREAMER_VERSION https://gitlab.freedesktop.org/gstreamer/gstreamer.git (Join-Path $WorkRoot "gstreamer")
    meson setup $GstBuild (Join-Path $WorkRoot "gstreamer") --prefix $GstRoot -Ddefault_library=shared -Dexamples=disabled -Dtests=disabled -Ddevtools=enabled -Ddoc=disabled
    meson compile -C $GstBuild
    meson install -C $GstBuild
} else {
    throw "unsupported GStreamerSource=$GstreamerSource"
}

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
