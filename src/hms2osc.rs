use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use huelib::{self, bridge::Bridge, resource::sensor::Sensor};
use log::{error, warn, info, debug};
use rosc::{OscMessage, OscPacket, OscType};
use serde::{Serialize, Deserialize};
use serde_json;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SensorKind {
    LightLevel,
    Presence,
    Temperature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensorConfig {
    pub name: String,
    #[serde(default = "SensorConfig::enabled_default")]
    pub enabled: bool,
    pub osc_address: String,
    #[serde(default = "SensorConfig::send_changes_only_default")]
    pub send_changes_only: bool,
}

impl SensorConfig {
    fn enabled_default() -> bool {
        true
    }

    fn send_changes_only_default() -> bool {
        false
    }
}

#[derive(Error, Debug)]
#[error(transparent)]
pub enum FromFileError {
    Io(#[from] io::Error),
    SerdeJson(#[from] serde_json::Error)
}

pub fn to_ip(host_str: &str) -> io::Result<IpAddr> {
    let first_addr = format!("{}:0", host_str).to_socket_addrs()?.next()
        .expect("to_socket_addrs() returned no addresses");
    Ok(first_addr.ip())
}

pub fn to_socket_addr(s: &str) -> io::Result<SocketAddr> {
    let first_addr = s.to_socket_addrs()?.next()
        .expect("to_socket_addrs() returned no addresses");
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
    let proj_dirs = ProjectDirs::from("fi", "pulusound", env!("CARGO_PKG_NAME"))
        .expect("failed to determine project directories");
    let path = proj_dirs.data_dir().join("username.txt");
    path
}

pub fn ensure_hue_user<P: AsRef<Path>>(bridge_host: IpAddr, user_file_path: P) -> Result<String,EnsureHueUserError> {
    let username = match fs::read_to_string(&user_file_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let name = register_hue_user(bridge_host)?;

            let parent_dir = user_file_path.as_ref().parent()
                .expect("failed to get parent directory of Hue user file path");
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
    pub kind: SensorKind,
    pub sensor_config: SensorConfig,
    pub osc_packet: OscPacket,
    pub last_osc_args: Vec<OscType>,
}

impl SensorTransformer {
    pub fn new(id: String, kind: SensorKind, sensor_config: SensorConfig) -> SensorTransformer {
        Self {
            id,
            kind,
            sensor_config: sensor_config.clone(),
            osc_packet: OscPacket::Message(OscMessage {
                addr: sensor_config.osc_address,
                args: Vec::new()
            }),
            last_osc_args: Vec::new()
        }
    }

    pub fn update(&mut self, bridge: &Bridge) -> Result<(), huelib::Error> {
        let OscPacket::Message(ref mut msg) = self.osc_packet else {
            unreachable!();
        };

        self.last_osc_args.clear();
        for arg in msg.args.iter() {
            self.last_osc_args.push(arg.clone());
        }

        msg.args.clear();
        let sensor = bridge.get_sensor(&self.id)?;
        debug!("update sensor {} ({})", self.id, self.sensor_config.name);

        match self.kind {
            SensorKind::LightLevel => {
                // light level, converted to lux
                msg.args.push(map_osc(
                    sensor.state.light_level,
                    |level| OscType::Float(10.0_f32.powf((level - 1) as f32 / 10000.0))
                ));

                msg.args.push(map_osc(
                    sensor.state.dark,
                    |dark| OscType::Float(if dark { 1.0 } else { 0.0 })
                ));

                msg.args.push(map_osc(
                    sensor.state.daylight,
                    |daylight| OscType::Float(if daylight { 1.0 } else { 0.0 })
                ));
            },
            SensorKind::Presence => {
                msg.args.push(map_osc(
                    sensor.state.presence,
                    |presence| OscType::Float(if presence { 1.0 } else { 0.0 })
                ));
            },
            SensorKind::Temperature => {
                msg.args.push(map_osc(
                    sensor.state.temperature,
                    |temperature| OscType::Float(temperature as f32 / 100.0)
                ));
            },
        }

        debug!("osc: {:?}", msg);

        if msg.args.iter().any(|arg| *arg == OscType::Nil) {
            warn!("failed to retrieve some data from {} ({}), kind={:?}", self.sensor_config.name, self.id, self.kind);
        }

        Ok(())
    }
}

impl fmt::Display for SensorTransformer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sc = &self.sensor_config;
        write!(f, "{} ({}), kind={:?} -> {}", sc.name, self.id, self.kind, sc.osc_address)
    }
}

pub fn prepare_sensor_transformers<'a>(sensors: &'a [Sensor], sensor_configs: &'a [SensorConfig]) -> impl Iterator<Item = SensorTransformer> + 'a {
    sensor_configs.iter()
        .filter(|sensor_config| sensor_config.enabled)
        .map(|sensor_config| {
            let sensor = sensors.iter()
                .filter(|s| s.name == sensor_config.name)
                .next()
                .unwrap_or_else(|| panic!("sensor {:?} not found", sensor_config.name));
            let id = sensor.id.clone();
            let kind = type_name_to_kind(&sensor.type_name)
                .unwrap_or_else(|| unimplemented!("{}", sensor.type_name));
            SensorTransformer::new(id, kind, sensor_config.clone())
        })
}

pub fn type_name_to_kind(type_name: &str) -> Option<SensorKind> {
    match type_name {
        "ZLLLightLevel" => Some(SensorKind::LightLevel),
        "ZLLPresence" => Some(SensorKind::Presence),
        "ZLLTemperature" => Some(SensorKind::Temperature),
        _ => None
    }
}

fn map_osc<T, F>(opt: Option<T>, f: F) -> OscType where F: FnOnce(T) -> OscType {
    match opt {
        Some(x) => f(x),
        None => OscType::Nil
    }
}
