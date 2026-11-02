//! Opt-in executable StackChan fixture check. All observed output is produced by guest drivers.
use esp_soc::{LoadKind,SocBus,Stop};

fn machine(image:&str)->esp32s3::Machine {
    let rom=std::env::var("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR to a directory containing esp32s3_rev0_rom.elf");
    let mut m=esp32s3::machine([2,0,0,0,0,1]);m.bus.board=esp32s3::board::make_board("stackchan").unwrap();m.bus.attach_board_devices();m.bus.set_flash_size(16<<20);m.bus.set_psram_size(8<<20).unwrap();m.console.capture=true;m.console.mask=2;
    m.load_input(LoadKind::Rom,&std::fs::read(format!("{rom}/esp32s3_rev0_rom.elf")).expect("ESP32SIM_ROM_DIR must contain a readable esp32s3_rev0_rom.elf")).unwrap();
    m.load_input(LoadKind::Flash,&std::fs::read(image).expect("ESP32SIM_STACKCHAN_FIXTURE or ESP32SIM_STACKCHAN_FACTORY must name a readable merged BIN")).unwrap();m.boot_rom();m
}
fn run(m:&mut esp32s3::Machine,ms:u64)->String {m.console.all.clear();m.max_cycles=m.bus.cycles+ms*240_000;let stop=m.run(u64::MAX);assert!(matches!(stop,Stop::Halted),"unexpected stop: {stop:?}");String::from_utf8_lossy(&m.console.all).into_owned()}
fn press(m:&mut esp32s3::Machine,id:&str,down:bool) {assert!(m.bus.board.button_at(m.bus.cycles,id,down),"unknown board control {id}");}

#[test]
#[ignore="requires ESP32SIM_STACKCHAN_FIXTURE (examples/platform-stackchan merged BIN) and ESP32SIM_ROM_DIR"]
fn external_stackchan_guest_touch_drives_rgb_display_and_serial_servos() {
    let image=std::env::var("ESP32SIM_STACKCHAN_FIXTURE").expect("set ESP32SIM_STACKCHAN_FIXTURE to the merged BIN built from examples/platform-stackchan");
    let mut m=machine(&image);let boot=run(&mut m,2000);
    for check in ["expander-version","yaw-goal-feedback","pitch-goal-feedback","wrong-tx-no-response","motor-power-off-no-response","motor-power-recovery-response"] {assert!(boot.contains(&format!("CHECK {check} PASS")),"{check}: {boot}");}
    assert!(boot.contains("PLATFORM_READY stackchan"));let initial=m.bus.board.display().unwrap();assert_eq!((initial.0,initial.1),(320,240));assert!(initial.2.contains(&0xffff));
    for (id,text,color,axis,angle) in [
        ("touch-pad-front","front=3 middle=0 back=0",[255,0,0],"pan","40.0000"),
        ("touch-pad-middle","front=0 middle=3 back=0",[0,255,0],"tilt","60.0000"),
        ("touch-pad-back","front=0 middle=0 back=3",[0,0,255],"pan","-40.0000"),
    ] {
        press(&mut m,id,true);let output=run(&mut m,400);assert!(output.contains(text),"{id}: {output}");assert!(m.bus.board.leds().unwrap().0.iter().all(|rgb|*rgb==color));
        let motion=m.bus.board.motion_snapshot().unwrap();assert!(motion.contains(&format!("\"id\":\"{axis}\",\"positionDegrees\":{angle}")),"{motion}");
        press(&mut m,id,false);let output=run(&mut m,400);assert!(output.contains("front=0 middle=0 back=0"));
    }
    press(&mut m,"touch-pad-front",true);press(&mut m,"touch-pad-middle",true);let output=run(&mut m,400);assert!(output.contains("front=3 middle=3 back=0"));
    press(&mut m,"touch-pad-front",false);press(&mut m,"touch-pad-middle",false);run(&mut m,400);
    m.bus.touch_input(120,100,true);let output=run(&mut m,200);assert!(output.contains("TOUCH_SCREEN count=1 x=120 y=100"),"{output}");assert_eq!(m.bus.board.display().unwrap().2[145*320+150],0xffe0);
    m.bus.touch_input(120,100,false);let output=run(&mut m,200);assert!(output.contains("TOUCH_SCREEN count=0"));assert_eq!(m.bus.board.display().unwrap().2[145*320+150],0xffff);
    // A reconstructed platform instance starts without inherited visible LED data.
    let fresh=machine(&image);assert!(fresh.bus.board.leds().unwrap().0.iter().all(|rgb|*rgb==[0;3]));
}

#[test]
#[ignore="requires ESP32SIM_STACKCHAN_FACTORY (official V1.5.1 merged BIN) and ESP32SIM_ROM_DIR"]
fn external_stackchan_factory_servo_test_moves_after_idle_torque_release() {
    let image=std::env::var("ESP32SIM_STACKCHAN_FACTORY").expect("set ESP32SIM_STACKCHAN_FACTORY to the official StackChan V1.5.1 merged BIN");
    let mut m=machine(&image);let boot=run(&mut m,3000);assert!(boot.contains("start startup worker"),"factory did not reach first-boot setup");
    // Welcome Start, then Servo Test Start, using the actual FT6336 screen path.
    m.bus.touch_input(235,188,true);run(&mut m,300);m.bus.touch_input(235,188,false);run(&mut m,2700);
    m.bus.touch_input(235,188,true);run(&mut m,300);m.bus.touch_input(235,188,false);
    let mut stage=0;let mut minimum=[f64::INFINITY;2];let mut maximum=[f64::NEG_INFINITY;2];let mut enabled=false;let mut angles=[0.0;2];
    for _ in 0..400 {
        run(&mut m,40);let motion=m.bus.board.motion_snapshot().unwrap();
        for (i,text) in motion.split("\"positionDegrees\":").skip(1).enumerate() {
            let value:f64=text.split(',').next().unwrap().parse().unwrap();angles[i]=value;minimum[i]=minimum[i].min(value);maximum[i]=maximum[i].max(value);
        }
        enabled|=motion.contains("\"enabled\":true");
        let reached=match stage {0=>angles[0]>85.0,1=>angles[0].abs()<1.0,2=>angles[0]< -85.0,3=>angles[0].abs()<1.0,4=>angles[1]>80.0,5=>angles[1]<4.0,_=>false};
        if reached {stage+=1;}
    }
    assert_eq!(stage,6,"factory must actually move left/home/right/home/up/home; min={minimum:?}, max={maximum:?}");assert!(enabled);
    assert!(angles[0].abs()<1.0 && angles[1]<4.0,"factory must return home: {angles:?}");
    assert!(!m.bus.board.motion_snapshot().unwrap().contains("\"enabled\":true"),"factory must release torque again at rest");
    let display=m.bus.board.display().unwrap();assert_eq!((display.0,display.1),(320,240));assert!(display.2.iter().any(|p|*p!=0));
}

// Protocol from m5stack/StackChan@f901cc2969189f3f0e96af1442c0f96e77e1e2c1:
// firmware/main/hal/hal_ble.cpp (10-byte fragment header), hal/utils/bleprph/bleprph.h
// (UUIDs), stackchan/json/json_helper.cpp and addons/neon_light/neon_light.cpp.
#[test]
#[ignore="requires ESP32SIM_STACKCHAN_FACTORY (official V1.5.1 merged BIN), ESP32SIM_STACKCHAN_SYMBOLS (verified profile ELF) and ESP32SIM_ROM_DIR"]
fn external_stackchan_factory_dance_gatt_drives_display_servos_and_rgb() {
    let image=std::env::var("ESP32SIM_STACKCHAN_FACTORY").expect("set ESP32SIM_STACKCHAN_FACTORY to the pinned official StackChan V1.5.1 merged BIN");
    let symbols=std::env::var("ESP32SIM_STACKCHAN_SYMBOLS").expect("set ESP32SIM_STACKCHAN_SYMBOLS to the verified stackchan-factory-1.5.1/symbols.elf");
    let elf=esp_soc::elf::parse(&std::fs::read(symbols).expect("ESP32SIM_STACKCHAN_SYMBOLS must name a readable ELF")).unwrap();
    let mut m=machine(&image);m.bus.enable_ble(&elf).unwrap();
    let mut serial=run(&mut m,3000);
    let tap=|m:&mut esp32s3::Machine,x,y| {m.bus.touch_input(x,y,true);let mut out=run(m,300);m.bus.touch_input(x,y,false);out+=&run(m,700);out};
    serial+=&tap(&mut m,88,188);
    for _ in 0..5 {serial+=&tap(&mut m,294,120);}
    serial+=&run(&mut m,1000);serial+=&tap(&mut m,160,120);serial+=&run(&mut m,1500);
    assert!(serial.contains("[DANCE] on open")&&serial.contains("[HAL-BLE] init done"),"official DANCE must open and initialize BLE");
    assert!(m.bus.ble.session.is_advertising(),"the real guest must enable advertising");
    // No implicit phone: one explicit, unencrypted packet-level virtual central.
    m.bus.ble_command("connect").unwrap();serial+=&run(&mut m,1000);
    m.bus.ble_command("discover").unwrap();serial+=&run(&mut m,1000);
    assert_eq!(m.bus.ble_pending_commands(),0,"real guest ATT discovery must finish");
    let before=m.bus.board.display().unwrap().2.to_vec();
    // Handles were obtained by discovery on this exact fixed image, not written into guest RAM.
    // Each ATT write is <=20 bytes; longer JSON uses the manufacturer's own fragment assembly.
    for (handle,json) in [(0x10u16,r#"{"yawServo":{"angle":30}}"#),(0x10,r#"{"pitchServo":{"angle":20}}"#),
        (0x13,r#"{"mouth":{"size":60}}"#),(0x19,r##"{"leftRgbColor":"#ff0000","rightRgbColor":"#0000ff"}"##)] {
        let bytes=json.as_bytes();let count=bytes.len().div_ceil(10);
        for (index,chunk) in bytes.chunks(10).enumerate() {
            let mut packet=vec![0xaa,0x55,0xc3,1];packet.extend_from_slice(&(index as u16).to_be_bytes());packet.extend_from_slice(&(count as u16).to_be_bytes());packet.extend_from_slice(&(bytes.len() as u16).to_be_bytes());packet.extend_from_slice(chunk);
            let hex=esp_soc::ble::peer::hex(&packet);m.bus.ble_command(&format!("write 0x{handle:04x} {hex}")).unwrap();serial+=&run(&mut m,200);
        }
    }
    serial+=&run(&mut m,2000);assert_eq!(m.bus.ble_pending_commands(),0);
    assert!(!serial.contains("Unknown HCI Command"),"all commands issued by this real startup/connection must be supported: {serial}");
    assert!(!serial.contains("deserializeJson failed")&&!serial.contains("fragment sequence mismatch"),"official JSON decoder must consume complete payloads");
    let motion=m.bus.board.motion_snapshot().unwrap();
    let angles:Vec<f64>=motion.split("\"positionDegrees\":").skip(1).map(|s|s.split(',').next().unwrap().parse().unwrap()).collect();
    assert_eq!(angles.len(),2);assert!((angles[0]-30.0).abs()<1.0&&(angles[1]-20.0).abs()<1.0,"actual serial servo positions: {motion}");
    let leds=m.bus.board.leds().unwrap().0;assert_eq!(leds.len(),12);assert!(leds[..6].iter().all(|v|*v==[255,0,0])&&leds[6..].iter().all(|v|*v==[0,0,255]),"actual PY32 RGB output: {leds:?}");
    let display=m.bus.board.display().unwrap();assert_ne!(display.2,before,"actual avatar framebuffer must change");
    assert!(display.2[145*320..146*320].iter().filter(|p|**p==0xffff).count()>70,"mouth size command must visibly expand the rendered mouth");
}
