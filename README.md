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

to view the full list of supported command-line options, run `hms2osc -h`:

```
Usage: hms2osc [OPTIONS] --config <FILE>

Options:
  -c, --config <FILE>  Set a config file
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
- `kind`: the kind of data provided by this sensor (`Presence`, `Temperature` or `AmbientLight`)

## OSC data

data from the different sensor kinds will be converted to OSC as follows:

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
