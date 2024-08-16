use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use huelib::{self, resource::sensor::Sensor};
use rosc::encoder;
use rosc::{OscMessage, OscPacket};
use serde::{Serialize, Deserialize};
use serde_json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SensorKind {
    AmbientLight,
    Presence,
    Temperature,
}

impl SensorKind {
    pub fn get_data(&self, sensor: &Sensor) -> () {
        match self {
            SensorKind::AmbientLight => {
                println!("ambient light: light_level={:?}, dark={:?}, daylight={:?}", sensor.state.light_level, sensor.state.dark, sensor.state.daylight);
                // conversion to lux:
                // lx = round(float(10 ** ((lightlevel - 1) / 10000))
            },
            SensorKind::Presence => {
                println!("presence: presence={:?}", sensor.state.presence);
            },
            SensorKind::Temperature => {
                println!("temperature: temperature={:?}", sensor.state.temperature);
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorConfig {
    pub name: String,
    pub kind: SensorKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub bridge_host: IpAddr,
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
    let sensors = config.sensors.iter()
        .map(|sensor_config| {
            let id = &all_sensors.iter()
                .filter(|s| s.name == sensor_config.name)
                .next().unwrap().id;
            (id, sensor_config)
        })
        .collect::<Vec<_>>();

    loop {
        for (id, sensor_config) in sensors.iter() {
            let sensor = bridge.get_sensor(*id).unwrap();
            // let presence = sensor.state.presence;
            // println!("sensor: {:?}", sensor);
            println!("sensor {} ({})", sensor_config.name, id);
            sensor_config.kind.get_data(&sensor);
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
