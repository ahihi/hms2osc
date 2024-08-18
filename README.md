# hms2osc

converts Hue motion sensor data to OSC messages.

## compatibility

any OS with Rust support should work. requires one or more Hue motion sensors connected to a Hue bridge.

## installation

download a prebuilt binary from [Releases](https://github.com/ahihi/hms2osc/releases), or [build](#building) it yourself!

## usage

prepare a [configuration file](#configuration) (named `config.json` in this example) and run:

```shell
hms2osc -c config.json
```

to see what sensors are available, run with `--list`:

```
hms2osc -c config.json --list
```

example output:

```
[*] available sensors:
 |  Hue ambient light sensor 1 (6), kind=LightLevel (type_name=ZLLLightLevel)
 |    State { presence: None, flag: None, last_updated: Some(2024-08-18T01:20:40), button_event: None, temperature: None, light_level: Some(4370), dark: Some(true), daylight: Some(false) }
 |  Daylight (1), kind=? (type_name=Daylight)
 |    State { presence: None, flag: None, last_updated: None, button_event: None, temperature: None, light_level: None, dark: None, daylight: None }
 |  Hue temperature sensor 1 (7), kind=Temperature (type_name=ZLLTemperature)
 |    State { presence: None, flag: None, last_updated: Some(2024-08-18T01:23:45), button_event: None, temperature: Some(2561), light_level: None, dark: None, daylight: None }
 |  Hue motion sensor 1 (5), kind=Presence (type_name=ZLLPresence)
 |    State { presence: Some(true), flag: None, last_updated: Some(2024-08-18T01:25:04), button_event: None, temperature: None, light_level: None, dark: None, daylight: None }
```

to view the full list of supported command-line options, run `hms2osc -h`:

```
Usage: hms2osc [OPTIONS] --config <FILE>

Options:
  -c, --config <FILE>  Set a config file
      --list           List available sensors
  -l, --log <LOG>      Set logging level
  -h, --help           Print help
  -V, --version        Print version
```

the logging level defaults to `info`. you can also set it to `debug` or `trace` to get more debugging information.

## configuration

see [config.json](config.json) for an example configuration.

the configuration is a JSON object with the following properties:

- `bridge_host`: IP address (or hostname) of the Hue bridge
- `osc_out_addr`: IP address (or hostname) and port to which OSC messages should be sent, separated by `:`
- `poll_interval`: number of seconds to wait between successive polls of the sensors
- `sensors`: list of sensor configurations

### sensor configuration

- `name`: human-readable name of the sensor, as specified on the Hue bridge
- `enabled`: whether or not to process data from this sensor
- `osc_address`: OSC address where data from this sensor should be sent

## OSC data

data from different sensor kinds will be converted to OSC as follows:

### `Presence`

1. presence (float): 1.0 if presence was detected, 0.0 otherwise

### `Temperature`

1. temperature (float): temperature in degrees Celsius

### `AmbientLight`

1. lux (float): ambient light level in lux
2. dark (float): 1.0 if the sensor's "dark" flag is set, 0.0 otherwise
3. daylight (float): 1.0 if the sensor's "daylight" flag is set, 0.0 otherwise

## building

you will need:

- rustc (tested with 1.80.1)
- Cargo

```shell
cd hms2osc
cargo build --release
```

this creates a stand-alone executable under `target/release` called `hms2osc`, which can be placed wherever you like.
