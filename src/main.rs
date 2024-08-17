use std::fs::File;
use std::io::BufReader;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use clap::Parser;
use colog;
use huelib;
use log::{info};
use rosc;
use serde::{Serialize, Deserialize};
use serde_json;

mod hms2osc;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Options {
    /// Set a config file
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    /// Set logging level
    #[arg(short, long)]
    log: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub bridge_host: IpAddr,
    pub osc_out_addr: SocketAddrV4,
    pub poll_interval: f32,
    pub sensors: Vec<hms2osc::SensorConfig>,
}

fn main() {
    let options = Options::parse();

    let mut colog_builder = colog::default_builder();
    if let Some(ref filters_str) = options.log {
        colog_builder.parse_filters(filters_str);
    }
    colog_builder.init();

    info!("config file path: {:?}", options.config);

    let config_file = File::open(&options.config).unwrap();
    let config_reader = BufReader::new(config_file);
    let config: Config = serde_json::from_reader(config_reader).unwrap();
    info!("bridge host: {}", config.bridge_host);
    info!("osc output address: {}", config.osc_out_addr);

    info!("connecting to bridge");
    let user_file_path = hms2osc::default_hue_user_file_path();
    let username = hms2osc::ensure_hue_user(config.bridge_host, user_file_path).unwrap();

    let bridge = huelib::bridge::Bridge::new(config.bridge_host, username);
    let all_sensors = bridge.get_all_sensors().unwrap();
    let mut sensor_tfs = hms2osc::prepare_sensor_transformers(&all_sensors, &config.sensors)
        .collect::<Vec<_>>();

    let sensors_str = sensor_tfs.iter()
        .map(|sensor_tf| format!("{}", sensor_tf))
        .collect::<Vec<_>>().join("\n");
    info!("enabled sensors:\n{}", sensors_str);

    let sock = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)).unwrap();
    let poll_interval = Duration::from_secs_f32(config.poll_interval);

    info!("polling sensors every {} seconds", poll_interval.as_secs());
    loop {
        for sensor_tf in sensor_tfs.iter_mut() {
            sensor_tf.update(&bridge).unwrap();

            let msg_buf = rosc::encoder::encode(&sensor_tf.osc_packet).unwrap();
            sock.send_to(&msg_buf, config.osc_out_addr).unwrap();
        }

        thread::sleep(poll_interval);
    }
}
