pub fn receiver_args(port: u16) -> Vec<String> {
    vec![
        "-q".into(),
        "udpsrc".into(),
        format!("port={port}"),
        "caps=application/x-rtp,media=audio,payload=96,clock-rate=44100,encoding-name=L16,format=S16BE,channels=2".into(),
        "!".into(),
        "rtpL16depay".into(),
        "!".into(),
        "audioconvert".into(),
        "!".into(),
        "audioresample".into(),
        "!".into(),
        "autoaudiosink".into(),
        "sync=false".into(),
    ]
}
