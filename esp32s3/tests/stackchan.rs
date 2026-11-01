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
