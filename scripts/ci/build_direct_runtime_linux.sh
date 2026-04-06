#!/usr/bin/env bash
set -euo pipefail

target="$1"
uxplay_builder="$2"
gstreamer_source="$3"
out_dir="$4"
beacon_helper_relpath="$5"

: "${UXPLAY_REF:?UXPLAY_REF must be set}"
: "${GSTREAMER_VERSION:?GSTREAMER_VERSION must be set}"

workspace_root="$(pwd)"
work_root="${workspace_root}/.runtime-cache/${target}"
uxplay_src="${work_root}/UxPlay"
uxplay_build="${work_root}/uxplay-build"
gst_src="${work_root}/gstreamer"
gst_build="${work_root}/gstreamer-build"
gst_prefix="${work_root}/gst-root"

rm -rf "${uxplay_src}" "${uxplay_build}" "${gst_src}" "${gst_build}" "${gst_prefix}"
mkdir -p "${work_root}"

git clone --depth 1 --branch "${UXPLAY_REF}" https://github.com/FDH2/UxPlay.git "${uxplay_src}"

cmake_args=(
  -S "${uxplay_src}"
  -B "${uxplay_build}"
  -DCMAKE_BUILD_TYPE=Release
)

if [[ "${uxplay_builder}" == "cross" ]]; then
  cat > "${work_root}/meson-cross.ini" <<'EOF'
[binaries]
c = 'aarch64-linux-gnu-gcc'
cpp = 'aarch64-linux-gnu-g++'
ar = 'aarch64-linux-gnu-ar'
strip = 'aarch64-linux-gnu-strip'
pkgconfig = 'pkg-config'

[host_machine]
system = 'linux'
cpu_family = 'aarch64'
cpu = 'aarch64'
endian = 'little'
EOF
  cmake_args+=(
    -DCMAKE_SYSTEM_NAME=Linux
    -DCMAKE_SYSTEM_PROCESSOR=aarch64
    -DCMAKE_C_COMPILER=aarch64-linux-gnu-gcc
    -DCMAKE_CXX_COMPILER=aarch64-linux-gnu-g++
  )
fi

cmake "${cmake_args[@]}"
cmake --build "${uxplay_build}" --parallel

if [[ "${gstreamer_source}" != "source" ]]; then
  echo "unsupported Linux gstreamer_source=${gstreamer_source}" >&2
  exit 1
fi

git clone --depth 1 --branch "${GSTREAMER_VERSION}" https://gitlab.freedesktop.org/gstreamer/gstreamer.git "${gst_src}"

meson_args=(
  setup "${gst_build}" "${gst_src}"
  --prefix "${gst_prefix}"
  -Ddefault_library=shared
  -Dexamples=disabled
  -Dtests=disabled
  -Ddevtools=enabled
  -Ddoc=disabled
)

if [[ "${uxplay_builder}" == "cross" ]]; then
  meson_args+=(--cross-file "${work_root}/meson-cross.ini")
fi

meson "${meson_args[@]}"
meson compile -C "${gst_build}"
meson install -C "${gst_build}"

python3 "${workspace_root}/scripts/prepare_direct_runtime.py" \
  --target "${target}" \
  --out-dir "${out_dir}" \
  --uxplay-path "${uxplay_build}/uxplay" \
  --gst-root "${gst_prefix}" \
  --beacon-script "${uxplay_src}/Bluetooth_LE_beacon/uxplay-beacon.py" \
  --beacon-helper-relpath "${beacon_helper_relpath}" \
  --python-path "python3" \
  --uxplay-version "${UXPLAY_REF}" \
  --gstreamer-version "${GSTREAMER_VERSION}"
