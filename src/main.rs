use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use huelib::{self, resource::sensor::Sensor};
use rosc::{encoder, OscMessage, OscPacket, OscType};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SensorKind {
    AmbientLight,
    Presence,
    Temperature,
}

impl SensorKind {
    pub fn push_data(&self, sensor: &Sensor, args: &mut Vec<OscType>) -> () {
        match self {
            SensorKind::AmbientLight => {
                let light_level = sensor.state.light_level.unwrap();
                let dark = sensor.state.dark.unwrap();
                let daylight = sensor.state.daylight.unwrap();
                let lux = 10.0_f32.powf((light_level - 1) as f32 / 10000.0);

                args.push(OscType::Float(lux));
                args.push(OscType::Float(if dark { 1.0 } else { 0.0 }));
                args.push(OscType::Float(if daylight { 1.0 } else { 0.0 }));
            },
            SensorKind::Presence => {
                let presence = sensor.state.presence.unwrap();

                args.push(OscType::Float(if presence { 1.0 } else { 0.0 }));
            },
            SensorKind::Temperature => {
                let temperature = sensor.state.temperature.unwrap();

                args.push(OscType::Float(temperature as f32 / 100.0));
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorConfig {
    pub name: String,
    pub enabled: bool,
    pub osc_address: String,
    pub kind: SensorKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub bridge_host: IpAddr,
    pub osc_out_addr: SocketAddrV4,
    pub poll_interval: f32,
    pub sensors: Vec<SensorConfig>,
}

fn config_path() -> io::Result<PathBuf> {
    let mut dir = env::current_exe()?;
    dir.pop();
    dir.push("config.json");
    Ok(dir)
}

fn main() {
    let config_file_path = "./config.json";//config_path().unwrap();
    println!("config_file_path: {:?}", config_file_path);
    let config_file = File::open(&config_file_path).unwrap();
    let config_reader = BufReader::new(config_file);
    let config: Config = serde_json::from_reader(config_reader).unwrap();

    let addr = config.bridge_host;
    // let sensor_name = "Hue motion sensor 1";
    let poll_interval = Duration::from_secs_f32(config.poll_interval);

    let proj_dirs = ProjectDirs::from("fi", "pulusound", "hms2osc").unwrap();
    let username_path = proj_dirs.data_dir().join("username");

    let username = match fs::read_to_string(&username_path) {
        Ok(u) => {
            println!("connecting to Hue bridge with username from {:?}", username_path);
            u
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(&username_path.parent().unwrap()).unwrap();
            let name = register_hue_user(addr).unwrap();
            let mut file = fs::File::create(&username_path).unwrap();
            write!(file, "{}", name).unwrap();
            name
        },
        e@Err(_) => e.unwrap()
    };

    let bridge = huelib::bridge::Bridge::new(addr, username);
    let all_sensors = bridge.get_all_sensors().unwrap();
    println!("all_sensors: {:?}", all_sensors);
    let mut sensors = config.sensors.iter()
        .filter(|sensor_config| sensor_config.enabled)
        .map(|sensor_config| {

            let id = &all_sensors.iter()
                .filter(|s| s.name == sensor_config.name)
                .next().unwrap().id;
            let osc_packet = OscPacket::Message(OscMessage {
                addr: sensor_config.osc_address.clone(),
                args: Vec::new()
            });
            (id, sensor_config, osc_packet)
        })
        .collect::<Vec<_>>();

    let host_addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0);
    let sock = UdpSocket::bind(host_addr).unwrap();

    loop {
        for (id, sensor_config, ref mut osc_packet) in sensors.iter_mut() {
            let OscPacket::Message(ref mut osc_msg) = osc_packet else {
                unreachable!();
            };
            osc_msg.args.clear();
            let sensor = bridge.get_sensor(*id).unwrap();
            println!("sensor {} ({})", sensor_config.name, id);
            sensor_config.kind.push_data(&sensor, &mut osc_msg.args);
            println!("osc_msg: {:?}", osc_msg);

            let msg_buf = encoder::encode(&osc_packet).unwrap();
            sock.send_to(&msg_buf, config.osc_out_addr).unwrap();
        }
        thread::sleep(poll_interval);
    }
}

fn register_hue_user(addr: IpAddr) -> huelib::Result<String> {
    let device_type = "hms2osc";
    println!("registering user on Hue bridge");
    match huelib::bridge::register_user(addr, device_type) {
        Err(huelib::Error::Response(e)) if e.kind == huelib::response::ErrorKind::LinkButtonNotPressed => {
            println!("waiting for the link button to be pressed");
            let mut result = None;
            while result.is_none() {
                result = match huelib::bridge::register_user(addr, device_type) {
                    Err(huelib::Error::Response(e)) if e.kind == huelib::response::ErrorKind::LinkButtonNotPressed => {
                        thread::sleep(Duration::from_secs(1));
                        None
                    },
                    r => Some(r)
                }
            }
            result.unwrap()
        },
        r => r
    }
}
