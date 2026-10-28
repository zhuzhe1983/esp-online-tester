// `--ble-nearby` is checked with the other arguments, before the ROM and the firmware load: it
// needs `--ble`, and a bad or colliding spec exits with status 2 and says why.
#[test]
fn ble_nearby_specs_are_checked_before_the_run() {
    for (args, message) in [
        (&["--ble-nearby", "name=Thermo"][..], "--ble-nearby needs the virtual controller of --ble"),
        (&["--ble", "--ble-nearby", "rssi=-60"][..], "--ble-nearby: a nearby device needs a name or advertising data"),
        (&["--ble", "--ble-nearby", "name=a,addr=02:53:49:00:00:02"][..], "--ble-nearby: nearby device 02:53:49:00:00:02 has the virtual peer's address"),
        (&["--ble", "--ble-nearby", "name=Thermo", "--ble-nearby", "name=Thermo,rssi=-90"][..], "share the address"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_esp32sim")).args(args).args(["--max-insns", "1"]).output().unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}
