//! Advisory checks from guest transactions, not inferred firmware or a silicon certification.
//! PM1 sources and CO5300 default conflicts are documented in docs/platform/hardware-diagnostics.md.
use super::PanelState;
use esp_periph::i2c::m5::M5pm1State;

// All string arguments below are internal ASCII text without JSON metacharacters.
fn finding(out:&mut Vec<String>,code:&str,severity:&str,count:u64,summary:&str,detail:&str) {
    out.push(format!(r#"{{"code":"{code}","severity":"{severity}","count":{},"summary":"{summary}","detail":"{detail}"}}"#,count.min(9_007_199_254_740_991)));
}
fn hz(value:Option<u32>)->String {value.map_or_else(||"null".into(),|n|n.to_string())}

pub(super) fn snapshot(pmic:&M5pm1State,panel:&PanelState)->String {
    let state=panel.panel.display_state().expect("StopWatch enables display state");
    let mut findings=Vec::new();
    if pmic.i2c_speed_mismatches>0 {
        finding(&mut findings,"PM1_I2C_CLOCK_MISMATCH","warning",pmic.i2c_speed_mismatches,
            "I2C clock exceeded the PM1 configuration",
            &format!("Observed {} Hz while PM1 register 0x09 allowed {} Hz. Use 100 kHz for initial communication and change both sides coherently. Real hardware may NACK; this observation does not inject a failure.",pmic.i2c_last_mismatch_hz.unwrap_or(0),pmic.i2c_last_mismatch_limit_hz.unwrap_or(0)));
    }
    if pmic.download_locked {
        finding(&mut findings,"PM1_DOWNLOAD_LOCK","warning",pmic.download_lock_writes.max(1),
            "Firmware enabled the download-button lock",
            "PM1 register 0x49 bit 7 is set. Long-press download can be disabled on hardware. Remove this write from normal initialization; an ESP reset does not establish that an existing physical lock has cleared.");
    }
    if pmic.download_unlock_attempts>0 {
        finding(&mut findings,"PM1_DOWNLOAD_UNLOCK_UNVERIFIED","warning",pmic.download_unlock_attempts,
            "A download-lock clear was requested",
            "The manufacturer library refuses clearing an already set lock until power cycling. This register model does not validate that a clear write unlocks a physical PM1.");
    }
    if !pmic.led_enabled {
        finding(&mut findings,"PM1_POWER_LED_DISABLED","info",1,
            "Firmware disabled the power indicator",
            "PM1 register 0x06 LED_EN is clear. This can be intentional, including in the factory firmware, and does not by itself mean the device is powered off.");
    }
    if panel.panel.pixels_written>0 && (!panel.active || !state.visible()) {
        finding(&mut findings,"CO5300_PIXELS_WHILE_HIDDEN","info",1,
            "Display memory has pixels but the panel is currently hidden",
            "Check OLED power/reset, sleep-out 0x11, display-on 0x29 and explicitly enabled brightness. Drawing before display-on can be intentional; raw display memory is not proof that the screen is lit.");
    }
    if state.pixel_bytes_control_unverified>0 && state.control_display.is_none() {
        finding(&mut findings,"CO5300_BRIGHTNESS_CONTROL","warning",1,
            "AMOLED brightness control was not explicitly initialized",
            "Pixels were sent without 0x53 WRCTRLD. CO5300 V0.01 gives conflicting reset defaults, so this is a check rather than a guaranteed failure. The official StopWatch sequence explicitly writes 0x53=0x20 and 0x51 brightness.");
    }
    if state.pixel_bytes_unsupported_format>0 {
        finding(&mut findings,"CO5300_PIXEL_FORMAT","warning",1,
            "Pixel data used a format outside this RGB565 model",
            "The CO5300 register 0x3A format at a pixel transfer was not RGB565. This model decodes two bytes per pixel; verify the configured format and actual wire payload before trusting its image.");
    }
    let control=state.brightness_control_enabled().map_or_else(||"null".into(),|v|v.to_string());
    format!(concat!(r#"{{"schemaVersion":1,"board":"stopwatch","state":{{"powerLedEnabled":{},"downloadLocked":{},"pm1ConfiguredHz":{},"pm1LastBusHz":{},"display":{{"sleepOut":{},"displayOn":{},"brightness":{},"brightnessControlEnabled":{},"visible":{}}}}},"findings":[{}],"limits":["PM1 wake/sleep delays, retry tolerance and analog power are not simulated. A successful simulation does not verify real hardware startup.","I2C rates are nominal controller settings. GPIO bit-banged clock rates and electrical signal quality are not measured.","CO5300 sleep/display and explicitly zero brightness affect visible output; physical brightness curves and uncertain reset defaults are not modeled."]}}"#),
        pmic.led_enabled,pmic.download_locked,pmic.i2c_speed_hz,hz(pmic.i2c_last_hz),state.sleep_out,state.display_on,state.brightness,control,panel.active&&state.visible(),findings.join(","))
}
