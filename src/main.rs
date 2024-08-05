use std::fs;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use huelib;
use rosc::encoder;
use rosc::{OscMessage, OscPacket};

fn main() {
    let addr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 7));
    let sensor_name = "Hue motion sensor 1";
    let poll_interval = Duration::from_secs(1);

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
    let sensors = bridge.get_all_sensors().unwrap();
    let sensor_id = &sensors.iter()
        .filter(|s| s.name == sensor_name)
        .next().unwrap().id;
    println!("sensors: {:?}", sensors);
    println!("sensor_id: {:?}", sensor_id);

    loop {
        let sensor = bridge.get_sensor(sensor_id).unwrap();
        let presence = sensor.state.presence;
        // println!("sensor: {:?}", sensor);
        println!("presence: {:?}", presence);
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
