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
meson_site_packages="${work_root}/meson-site-packages"

version_is_at_least() {
  python3 - "$1" "$2" <<'PY'
import re
import sys


def parse(version: str) -> tuple[int, ...]:
    parts = [int(component) for component in re.findall(r"\d+", version)]
    if not parts:
        raise SystemExit(1)
    return tuple(parts)


current = parse(sys.argv[1])
required = parse(sys.argv[2])
width = max(len(current), len(required))
current += (0,) * (width - len(current))
required += (0,) * (width - len(required))

raise SystemExit(0 if current >= required else 1)
PY
}

resolve_gstreamer_meson_requirement() {
  python3 - "$1" <<'PY'
from pathlib import Path
import re
import sys

text = Path(sys.argv[1]).read_text(encoding="utf-8")
match = re.search(r'meson_version\s*:\s*[\'"]([^\'"]+)[\'"]', text)
if not match:
    raise SystemExit("unable to determine GStreamer Meson version requirement")

constraint = match.group(1)
minimum = re.search(r'>=\s*([0-9][0-9.]*)', constraint)
if minimum:
    print(minimum.group(1))
    raise SystemExit(0)

exact = re.fullmatch(r'\s*([0-9][0-9.]*)\s*', constraint)
if exact:
    print(exact.group(1))
    raise SystemExit(0)

raise SystemExit(f"unsupported Meson version constraint: {constraint}")
PY
}

ensure_meson() {
  local required_version="$1"

  if command -v meson >/dev/null 2>&1; then
    local system_meson_version
    system_meson_version="$(meson --version)"
    if version_is_at_least "${system_meson_version}" "${required_version}"; then
      echo "Using system Meson ${system_meson_version}" >&2
      return
    fi
    echo "System Meson ${system_meson_version} is older than required ${required_version}; installing private Meson" >&2
  else
    echo "Meson not found on PATH; installing private Meson ${required_version}+." >&2
  fi

  rm -rf "${meson_site_packages}"
  python3 -m pip install --upgrade --disable-pip-version-check --target "${meson_site_packages}" "meson>=${required_version},<2"
}

run_meson() {
  if [[ -d "${meson_site_packages}" ]]; then
    PYTHONPATH="${meson_site_packages}${PYTHONPATH:+:${PYTHONPATH}}" \
      python3 -m mesonbuild.mesonmain "$@"
    return
  fi

  meson "$@"
}

rm -rf "${uxplay_src}" "${uxplay_build}" "${gst_src}" "${gst_build}" "${gst_prefix}" "${meson_site_packages}"
mkdir -p "${work_root}"

git clone --depth 1 --branch "${UXPLAY_REF}" https://github.com/FDH2/UxPlay.git "${uxplay_src}"

cmake_args=(
  -S "${uxplay_src}"
  -B "${uxplay_build}"
  -DCMAKE_BUILD_TYPE=Release
)

if [[ "${uxplay_builder}" == "cross" ]]; then
  case "${target}" in
    aarch64-unknown-linux-gnu)
      target_pkgconfig_dir="/usr/lib/aarch64-linux-gnu/pkgconfig"
      ;;
    *)
      echo "unsupported Linux cross target=${target}" >&2
      exit 1
      ;;
  esac

  # Point pkg-config at the target sysroot so CMake and Meson don't pick host .pc files.
  export PKG_CONFIG_ALLOW_CROSS=1
  export PKG_CONFIG_LIBDIR="${target_pkgconfig_dir}:/usr/share/pkgconfig"
  export PKG_CONFIG_PATH=
  export PKG_CONFIG_SYSROOT_DIR=/

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

if [[ "${gstreamer_source}" != "source" ]]; then
  echo "unsupported Linux gstreamer_source=${gstreamer_source}" >&2
  exit 1
fi

git clone --depth 1 --branch "${GSTREAMER_VERSION}" https://gitlab.freedesktop.org/gstreamer/gstreamer.git "${gst_src}"

gst_required_meson_version="$(resolve_gstreamer_meson_requirement "${gst_src}/meson.build")"
ensure_meson "${gst_required_meson_version}"

meson_args=(
  setup "${gst_build}" "${gst_src}"
  --prefix "${gst_prefix}"
  --libdir lib
  -Ddefault_library=shared
  -Dexamples=disabled
  -Dtests=disabled
  -Ddevtools=enabled
  -Ddoc=disabled
)

if [[ "${uxplay_builder}" == "cross" ]]; then
  meson_args+=(--cross-file "${work_root}/meson-cross.ini")
fi

run_meson "${meson_args[@]}"
run_meson compile -C "${gst_build}"
run_meson install -C "${gst_build}"

# UxPlay's CMake config uses pkg-config to locate GStreamer modules.
gst_pkgconfig_path="${gst_prefix}/lib/pkgconfig:${gst_prefix}/lib64/pkgconfig:${gst_prefix}/share/pkgconfig"
export PKG_CONFIG_PATH="${gst_pkgconfig_path}${PKG_CONFIG_PATH:+:${PKG_CONFIG_PATH}}"
export CMAKE_PREFIX_PATH="${gst_prefix}${CMAKE_PREFIX_PATH:+:${CMAKE_PREFIX_PATH}}"

cmake "${cmake_args[@]}"
cmake --build "${uxplay_build}" --parallel

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
