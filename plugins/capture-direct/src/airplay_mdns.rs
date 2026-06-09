use anyhow::{anyhow, Context, Result};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use plist::Value;
use std::io::{Cursor, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream, UdpSocket};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const AIRPLAY_TYPE: &str = "_airplay._tcp.local.";
const RAOP_TYPE: &str = "_raop._tcp.local.";
const HOSTNAME: &str = "ios-control.local.";
const FALLBACK_MODEL: &str = "AppleTV3,2";
const FALLBACK_SOURCE_VERSION: &str = "220.68";
const FALLBACK_VV: &str = "2";
const FALLBACK_FEATURES: u64 = 0x527F_FEE6;
const PI: &str = "2e388006-13ba-4041-9a67-25dd4a43d536";
const INFO_RETRY_WINDOW: Duration = Duration::from_secs(20);
const INFO_RETRY_INTERVAL: Duration = Duration::from_millis(250);
const INFO_IO_TIMEOUT: Duration = Duration::from_millis(600);

#[derive(Debug, Clone)]
pub struct AirPlayMdnsConfig {
    pub receiver_name: String,
    pub device_id: String,
    pub rtsp_port: u16,
}

pub struct AirPlayMdnsPublisher {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AirPlayServiceMetadata {
    name: String,
    device_id: String,
    device_compact: String,
    model: String,
    source_version: String,
    vv: String,
    features: String,
    pk_hex: String,
}

struct PublishedServices {
    daemon: ServiceDaemon,
    fullnames: Vec<String>,
}

impl AirPlayMdnsPublisher {
    pub fn start(config: AirPlayMdnsConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("airplay-mdns-publisher".into())
            .spawn(move || run_publisher(config, worker_stop))
            .ok();

        Self { stop, worker }
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for AirPlayMdnsPublisher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_publisher(config: AirPlayMdnsConfig, stop: Arc<AtomicBool>) {
    let deadline = Instant::now() + INFO_RETRY_WINDOW;
    let info_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, config.rtsp_port);

    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        match query_uxplay_info(info_addr, INFO_IO_TIMEOUT)
            .and_then(|info| metadata_from_plist(&info, &config))
            .and_then(|metadata| publish_services(&metadata, config.rtsp_port))
        {
            Ok(published) => {
                wait_until_stopped(&stop);
                published.shutdown();
                return;
            }
            Err(_) => sleep_or_stop(&stop, INFO_RETRY_INTERVAL),
        }
    }
}

fn wait_until_stopped(stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn sleep_or_stop(stop: &AtomicBool, duration: Duration) {
    let started = Instant::now();
    while !stop.load(Ordering::Relaxed) && started.elapsed() < duration {
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn query_uxplay_info(addr: SocketAddrV4, timeout: Duration) -> Result<Value> {
    let socket_addr = SocketAddr::V4(addr);
    let mut stream = TcpStream::connect_timeout(&socket_addr, timeout)
        .with_context(|| format!("failed to connect to UxPlay RTSP info at {addr}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(b"GET /info RTSP/1.0\r\nCSeq: 1\r\nUser-Agent: AirPlay/920.10.1\r\n\r\n")?;

    let mut data = Vec::with_capacity(64 * 1024);
    let mut buf = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buf)?;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&buf[..read]);
        if let Some(body) = rtsp_body(&data)? {
            return Value::from_reader(Cursor::new(body)).context("failed to parse UxPlay plist");
        }
        if data.len() > 1024 * 1024 {
            return Err(anyhow!("UxPlay /info response is too large"));
        }
    }

    Err(anyhow!(
        "UxPlay /info response did not include a complete plist"
    ))
}

fn rtsp_body(data: &[u8]) -> Result<Option<&[u8]>> {
    let Some(header_end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };
    let body_start = header_end + 4;
    let header = std::str::from_utf8(&data[..header_end]).context("RTSP header is not UTF-8")?;
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("UxPlay /info response missing Content-Length"))?;
    let body_end = body_start
        .checked_add(content_length)
        .ok_or_else(|| anyhow!("UxPlay /info content length overflow"))?;
    if data.len() < body_end {
        return Ok(None);
    }

    Ok(Some(&data[body_start..body_end]))
}

fn metadata_from_plist(info: &Value, config: &AirPlayMdnsConfig) -> Result<AirPlayServiceMetadata> {
    let dict = info
        .as_dictionary()
        .ok_or_else(|| anyhow!("UxPlay /info plist is not a dictionary"))?;
    let name = plist_string(dict, "name").unwrap_or_else(|| config.receiver_name.clone());
    let device_id = normalize_device_id(
        &plist_string(dict, "deviceID").unwrap_or_else(|| config.device_id.clone()),
    );
    let device_compact = device_id.replace(':', "");
    let model = plist_string(dict, "model").unwrap_or_else(|| FALLBACK_MODEL.into());
    let source_version =
        plist_string(dict, "sourceVersion").unwrap_or_else(|| FALLBACK_SOURCE_VERSION.into());
    let vv = plist_string(dict, "vv")
        .or_else(|| plist_u64(dict, "vv").map(|value| value.to_string()))
        .unwrap_or_else(|| FALLBACK_VV.into());
    let features_value = plist_u64(dict, "features").unwrap_or(FALLBACK_FEATURES);
    let pk_hex = plist_bytes(dict, "pk")
        .map(bytes_to_hex)
        .or_else(|| plist_string(dict, "pk").map(|value| value.trim_start_matches("0x").into()))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("UxPlay /info plist missing public key"))?;

    Ok(AirPlayServiceMetadata {
        name,
        device_id,
        device_compact,
        model,
        source_version,
        vv,
        features: format!("0x{features_value:X},0x0"),
        pk_hex,
    })
}

fn publish_services(metadata: &AirPlayServiceMetadata, port: u16) -> Result<PublishedServices> {
    let ip = local_ipv4().context("failed to determine local IPv4 address for AirPlay mDNS")?;
    let services = build_service_infos(metadata, ip, port)?;
    let daemon = ServiceDaemon::new().context("failed to start mDNS responder")?;
    let fullnames = services
        .iter()
        .map(|service| service.get_fullname().to_string())
        .collect::<Vec<_>>();

    for service in services {
        daemon
            .register(service)
            .context("failed to register AirPlay mDNS service")?;
    }

    Ok(PublishedServices { daemon, fullnames })
}

fn build_service_infos(
    metadata: &AirPlayServiceMetadata,
    ip: Ipv4Addr,
    port: u16,
) -> Result<Vec<ServiceInfo>> {
    let airplay_txt = vec![
        ("deviceid", metadata.device_id.clone()),
        ("features", metadata.features.clone()),
        ("flags", "0x4".into()),
        ("model", metadata.model.clone()),
        ("pk", metadata.pk_hex.clone()),
        ("pi", PI.into()),
        ("srcvers", metadata.source_version.clone()),
        ("vv", metadata.vv.clone()),
        ("pw", "false".into()),
    ];
    let raop_txt = vec![
        ("ch", "2".into()),
        ("cn", "0,1,2,3".into()),
        ("da", "true".into()),
        ("et", "0,3,5".into()),
        ("vv", "2".into()),
        ("ft", metadata.features.clone()),
        ("am", metadata.model.clone()),
        ("md", "0,1,2".into()),
        ("rhd", "5.6.0.0".into()),
        ("pw", "false".into()),
        ("sf", "0x4".into()),
        ("sr", "44100".into()),
        ("ss", "16".into()),
        ("sv", "false".into()),
        ("tp", "UDP".into()),
        ("txtvers", "1".into()),
        ("vs", metadata.source_version.clone()),
        ("vn", "65537".into()),
        ("pk", metadata.pk_hex.clone()),
    ];
    let address = IpAddr::V4(ip);
    let airplay = ServiceInfo::new(
        AIRPLAY_TYPE,
        &metadata.name,
        HOSTNAME,
        address,
        port,
        &airplay_txt[..],
    )?;
    let raop_name = format!("{}@{}", metadata.device_compact, metadata.name);
    let raop = ServiceInfo::new(
        RAOP_TYPE,
        &raop_name,
        HOSTNAME,
        address,
        port,
        &raop_txt[..],
    )?;

    Ok(vec![airplay, raop])
}

impl PublishedServices {
    fn shutdown(self) {
        for fullname in &self.fullnames {
            let _ = self.daemon.unregister(fullname);
        }
        if let Ok(status) = self.daemon.shutdown() {
            let _ = status.recv_timeout(Duration::from_millis(750));
        }
    }
}

fn local_ipv4() -> Result<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80))?;
    match socket.local_addr()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Ok(ip),
        ip => Err(anyhow!("no LAN IPv4 address found, got {ip}")),
    }
}

fn plist_string(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(Value::as_string)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn plist_u64(dict: &plist::Dictionary, key: &str) -> Option<u64> {
    let value = dict.get(key)?;
    value.as_unsigned_integer().or_else(|| {
        value
            .as_signed_integer()
            .and_then(|integer| u64::try_from(integer).ok())
    })
}

fn plist_bytes<'a>(dict: &'a plist::Dictionary, key: &str) -> Option<&'a [u8]> {
    dict.get(key).and_then(Value::as_data)
}

fn normalize_device_id(device_id: &str) -> String {
    device_id.trim().to_ascii_uppercase()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata_fixture() -> AirPlayServiceMetadata {
        AirPlayServiceMetadata {
            name: "iOS Control".into(),
            device_id: "02:10:50:00:00:01".into(),
            device_compact: "021050000001".into(),
            model: "AppleTV3,2".into(),
            source_version: "220.68".into(),
            vv: "2".into(),
            features: "0x527FFEE6,0x0".into(),
            pk_hex: "abcdef".into(),
        }
    }

    #[test]
    fn metadata_from_plist_uses_uxplay_values_and_hex_public_key() {
        let mut dict = plist::Dictionary::new();
        dict.insert("name".into(), Value::String("iOS Control".into()));
        dict.insert("deviceID".into(), Value::String("02:10:50:00:00:01".into()));
        dict.insert("model".into(), Value::String("AppleTV3,2".into()));
        dict.insert("sourceVersion".into(), Value::String("220.68".into()));
        dict.insert("vv".into(), Value::String("2".into()));
        dict.insert("features".into(), Value::Integer(0x527F_FEE6_u64.into()));
        dict.insert("pk".into(), Value::Data(vec![0xab, 0xcd, 0xef]));

        let metadata = metadata_from_plist(
            &Value::Dictionary(dict),
            &AirPlayMdnsConfig {
                receiver_name: "fallback".into(),
                device_id: "02:00:00:00:00:01".into(),
                rtsp_port: 52082,
            },
        )
        .unwrap();

        assert_eq!(metadata, metadata_fixture());
    }

    #[test]
    fn service_infos_match_airplay_and_raop_dns_sd_records() {
        let services =
            build_service_infos(&metadata_fixture(), Ipv4Addr::new(192, 168, 2, 94), 52082)
                .unwrap();
        let airplay = services
            .iter()
            .find(|service| service.get_type() == AIRPLAY_TYPE)
            .unwrap();
        let raop = services
            .iter()
            .find(|service| service.get_type() == RAOP_TYPE)
            .unwrap();

        assert_eq!(airplay.get_fullname(), "iOS Control._airplay._tcp.local.");
        assert_eq!(
            raop.get_fullname(),
            "021050000001@iOS Control._raop._tcp.local."
        );
        assert_eq!(airplay.get_port(), 52082);
        assert_eq!(raop.get_port(), 52082);
        assert_eq!(
            airplay.get_property_val_str("deviceid"),
            Some("02:10:50:00:00:01")
        );
        assert_eq!(
            airplay.get_property_val_str("features"),
            Some("0x527FFEE6,0x0")
        );
        assert_eq!(airplay.get_property_val_str("pk"), Some("abcdef"));
        assert_eq!(raop.get_property_val_str("ft"), Some("0x527FFEE6,0x0"));
        assert_eq!(raop.get_property_val_str("am"), Some("AppleTV3,2"));
        assert_eq!(raop.get_property_val_str("vs"), Some("220.68"));
    }

    #[test]
    fn rtsp_body_waits_for_complete_content_length() {
        let body = b"bplist00payload";
        let mut response = Vec::new();
        response.extend_from_slice(b"RTSP/1.0 200 OK\r\nContent-Length: ");
        response.extend_from_slice(body.len().to_string().as_bytes());
        response.extend_from_slice(b"\r\n\r\n");
        response.extend_from_slice(&body[..4]);
        assert!(rtsp_body(&response).unwrap().is_none());

        response.extend_from_slice(&body[4..]);
        assert_eq!(rtsp_body(&response).unwrap(), Some(body.as_slice()));
    }
}
