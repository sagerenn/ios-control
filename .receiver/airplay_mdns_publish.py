import argparse
import binascii
import plistlib
import socket
import time
from typing import Dict

from zeroconf import ServiceInfo, Zeroconf


def local_ipv4() -> str:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.connect(("8.8.8.8", 80))
        return sock.getsockname()[0]
    finally:
        sock.close()


def query_info(host: str, port: int) -> Dict[str, object]:
    request = (
        "GET /info RTSP/1.0\r\n"
        "CSeq: 1\r\n"
        "User-Agent: AirPlay/920.10.1\r\n"
        "\r\n"
    ).encode("ascii")
    sock = socket.create_connection((host, port), timeout=4)
    try:
        sock.sendall(request)
        sock.settimeout(4)
        data = bytearray()
        content_length = None
        while True:
            chunk = sock.recv(65536)
            if not chunk:
                break
            data.extend(chunk)
            if b"\r\n\r\n" not in data:
                continue
            header, body = bytes(data).split(b"\r\n\r\n", 1)
            if content_length is None:
                for line in header.splitlines():
                    if line.lower().startswith(b"content-length:"):
                        content_length = int(line.split(b":", 1)[1].strip())
                        break
            if content_length is not None and len(body) >= content_length:
                return plistlib.loads(body[:content_length])
    finally:
        sock.close()
    raise RuntimeError("UxPlay /info response did not include a complete plist")


def process_alive(pid: int) -> bool:
    if pid <= 0:
        return True
    try:
        import psutil

        return psutil.pid_exists(pid)
    except Exception:
        return True


def publish(args: argparse.Namespace) -> None:
    info = query_info(args.host, args.port)
    ip = args.ipv4 or local_ipv4()
    name = str(info.get("name") or args.name)
    device_id = str(info.get("deviceID") or args.device_id).upper()
    device_compact = device_id.replace(":", "")
    model = str(info.get("model") or "AppleTV3,2")
    source_version = str(info.get("sourceVersion") or "220.68")
    vv = str(info.get("vv") or "2")
    features_value = int(info.get("features") or 0x527FFEE6)
    features = f"0x{features_value:X},0x0"
    pk = info.get("pk")
    pk_hex = binascii.hexlify(pk).decode("ascii") if isinstance(pk, bytes) else str(pk)

    airplay_txt = {
        "deviceid": device_id,
        "features": features,
        "flags": "0x4",
        "model": model,
        "pk": pk_hex,
        "pi": "2e388006-13ba-4041-9a67-25dd4a43d536",
        "srcvers": source_version,
        "vv": vv,
        "pw": "false",
    }
    raop_txt = {
        "ch": "2",
        "cn": "0,1,2,3",
        "da": "true",
        "et": "0,3,5",
        "vv": "2",
        "ft": features,
        "am": model,
        "md": "0,1,2",
        "rhd": "5.6.0.0",
        "pw": "false",
        "sf": "0x4",
        "sr": "44100",
        "ss": "16",
        "sv": "false",
        "tp": "UDP",
        "txtvers": "1",
        "vs": source_version,
        "vn": "65537",
        "pk": pk_hex,
    }

    server = args.server if args.server.endswith(".") else f"{args.server}."
    address = socket.inet_aton(ip)
    services = [
        ServiceInfo(
            "_airplay._tcp.local.",
            f"{name}._airplay._tcp.local.",
            addresses=[address],
            port=args.port,
            properties=airplay_txt,
            server=server,
        ),
        ServiceInfo(
            "_raop._tcp.local.",
            f"{device_compact}@{name}._raop._tcp.local.",
            addresses=[address],
            port=args.port,
            properties=raop_txt,
            server=server,
        ),
    ]

    zeroconf = Zeroconf()
    try:
        for service in services:
            zeroconf.register_service(service, allow_name_change=True)
        print(f"Published AirPlay mDNS for {name} at {ip}:{args.port}", flush=True)
        while process_alive(args.watch_pid):
            time.sleep(1)
    finally:
        for service in services:
            try:
                zeroconf.unregister_service(service)
            except Exception:
                pass
        zeroconf.close()


parser = argparse.ArgumentParser()
parser.add_argument("--host", default="127.0.0.1")
parser.add_argument("--port", type=int, required=True)
parser.add_argument("--name", default="iOS Control")
parser.add_argument("--device-id", default="02:10:50:00:00:01")
parser.add_argument("--ipv4")
parser.add_argument("--server", default="ios-control.local.")
parser.add_argument("--watch-pid", type=int, default=0)
args = parser.parse_args()

publish(args)
