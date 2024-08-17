use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use huelib::{self, bridge::Bridge, resource::sensor::Sensor};
use log::{error, info, debug};
use rosc::{OscMessage, OscPacket, OscType};
use serde::{Serialize, Deserialize};
use serde_json;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SensorKind {
    AmbientLight,
    Presence,
    Temperature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorConfig {
    pub name: String,
    pub enabled: bool,
    pub osc_address: String,
    pub kind: SensorKind,
}

#[derive(Error, Debug)]
#[error(transparent)]
pub enum FromFileError {
    Io(#[from] io::Error),
    SerdeJson(#[from] serde_json::Error)
}

pub fn to_ip(host_str: &str) -> io::Result<IpAddr> {
    let first_addr = format!("{}:0", host_str).to_socket_addrs()?.next().unwrap();
    Ok(first_addr.ip())
}

pub fn to_socket_addr(s: &str) -> io::Result<SocketAddr> {
    let first_addr = s.to_socket_addrs()?.next().unwrap();
    Ok(first_addr)
}

pub fn to_bind_addr(dst_addr: SocketAddr) -> SocketAddr {
    match dst_addr {
        SocketAddr::V4(_) => SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0).into(),
        SocketAddr::V6(_) => SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0, 0, 0).into()
    }
}

pub fn register_hue_user(bridge_host: IpAddr) -> Result<String,huelib::Error> {
    let device_type = env!("CARGO_PKG_NAME");
    info!("registering user on Hue bridge");
    match huelib::bridge::register_user(bridge_host, device_type) {
        Err(huelib::Error::Response(e)) if e.kind == huelib::response::ErrorKind::LinkButtonNotPressed => {
            info!("waiting for the link button to be pressed...");
            let mut result = None;
            while result.is_none() {
                result = match huelib::bridge::register_user(bridge_host, device_type) {
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

pub fn default_hue_user_file_path() -> PathBuf {
    let proj_dirs = ProjectDirs::from("fi", "pulusound", env!("CARGO_PKG_NAME")).unwrap();
    let path = proj_dirs.data_dir().join("username.txt");
    path
}

pub fn ensure_hue_user<P: AsRef<Path>>(bridge_host: IpAddr, user_file_path: P) -> Result<String,EnsureHueUserError> {
    let username = match fs::read_to_string(&user_file_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let name = register_hue_user(bridge_host)?;

            let parent_dir = user_file_path.as_ref().parent().unwrap();
            fs::create_dir_all(&parent_dir)?;
            let mut file = fs::File::create(&user_file_path)?;
            write!(file, "{}", name)?;

            Ok(name)
        },
        r => r
    }?;
    Ok(username)
}

#[derive(Error, Debug)]
#[error(transparent)]
#[non_exhaustive]
pub enum EnsureHueUserError {
    Hue(#[from] huelib::Error),
    Io(#[from] io::Error)
}

#[derive(Debug)]
pub struct SensorTransformer {
    pub id: String,
    pub sensor_config: SensorConfig,
    pub osc_packet: OscPacket,
}

impl SensorTransformer {
    pub fn new(id: String, sensor_config: SensorConfig) -> SensorTransformer {
        Self {
            id,
            sensor_config: sensor_config.clone(),
            osc_packet: OscPacket::Message(OscMessage {
                addr: sensor_config.osc_address,
                args: Vec::new()
            })
        }
    }

    pub fn update(&mut self, bridge: &Bridge) -> Result<(), huelib::Error> {
        let OscPacket::Message(ref mut msg) = self.osc_packet else {
            unreachable!();
        };
        msg.args.clear();
        let sensor = bridge.get_sensor(&self.id)?;
        debug!("update sensor {} ({})", self.id, self.sensor_config.name);

        match self.sensor_config.kind {
            SensorKind::AmbientLight => {
                let light_level = sensor.state.light_level.unwrap();
                let lux = 10.0_f32.powf((light_level - 1) as f32 / 10000.0);
                let dark = sensor.state.dark.unwrap();
                let daylight = sensor.state.daylight.unwrap();

                msg.args.push(OscType::Float(lux));
                msg.args.push(OscType::Float(if dark { 1.0 } else { 0.0 }));
                msg.args.push(OscType::Float(if daylight { 1.0 } else { 0.0 }));
            },
            SensorKind::Presence => {
                let presence = sensor.state.presence.unwrap();

                msg.args.push(OscType::Float(if presence { 1.0 } else { 0.0 }));
            },
            SensorKind::Temperature => {
                let temperature = sensor.state.temperature.unwrap();

                msg.args.push(OscType::Float(temperature as f32 / 100.0));
            },
        }

        debug!("osc: {:?}", msg);

        Ok(())
    }
}

impl fmt::Display for SensorTransformer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sc = &self.sensor_config;
        write!(f, "{} ({}), kind={:?} -> {}", sc.name, self.id, sc.kind, sc.osc_address)
    }
}

pub fn prepare_sensor_transformers<'a>(sensors: &'a [Sensor], sensor_configs: &'a [SensorConfig]) -> impl Iterator<Item = SensorTransformer> + 'a {
    sensor_configs.iter()
        .filter(|sensor_config| sensor_config.enabled)
        .map(|sensor_config| {
            let id = sensors.iter()
                .filter(|s| s.name == sensor_config.name)
                .next().unwrap().id.clone();
            SensorTransformer::new(id, sensor_config.clone())
        })
}
