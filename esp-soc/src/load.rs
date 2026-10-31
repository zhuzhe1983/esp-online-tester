//! Input contracts shared by native and browser front ends.
use crate::{Machine, Soc, SocBus};

/// Parse the return value shared by CLI, browser and network stubs.
pub fn stub_spec(spec: &str) -> Result<(&str, u32), String> {
    let (name, value) = spec.split_once('=').unwrap_or((spec, "0"));
    if name.is_empty() { return Err("stub name must not be empty".into()); }
    let value = match value {
        "true" => 1,
        "false" => 0,
        value => match value.strip_prefix("0x") {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => value.parse(),
        }.map_err(|_| format!("invalid return value in {spec:?}: expected u32, true or false"))?,
    };
    Ok((name, value))
}

/// Stable load-kind numbers used by both browser ABIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LoadKind {
    Rom = 0,
    Bootloader = 1,
    Partitions = 2,
    App = 3,
    Symbols = 4,
    Flash = 5,
    Script = 6,
    /// BMP/PPM, or baseline JPEG for boards whose camera accepts encoded input
    CameraPicture = 7,
    /// a card image for the SD socket, held in memory
    SdCard = 8,
}

impl TryFrom<u32> for LoadKind {
    type Error = String;
    fn try_from(kind: u32) -> Result<Self, Self::Error> {
        match kind {
            0 => Ok(Self::Rom),
            1 => Ok(Self::Bootloader),
            2 => Ok(Self::Partitions),
            3 => Ok(Self::App),
            4 => Ok(Self::Symbols),
            5 => Ok(Self::Flash),
            6 => Ok(Self::Script),
            7 => Ok(Self::CameraPicture),
            8 => Ok(Self::SdCard),
            _ => Err(format!("unknown load kind {kind}")),
        }
    }
}

/// `Soc::check_image` for a whole flash image: its bootloader and the app at 0x10000, where the
/// loaders put one. The partition table is not read, so an app at another offset is not checked;
/// its bootloader still is.
pub fn check_flash_image<S: Soc>(flash: &[u8]) -> Result<(), String> {
    for (what, off) in [("bootloader", S::BOOTLOADER_OFFSET), ("app", 0x10000)] {
        if let Some(image) = flash.get(off..) { S::check_image(image).map_err(|e| format!("({what} at {off:#x}) {e}"))?; }
    }
    Ok(())
}

impl<S: Soc> Machine<S> {
    pub fn load_input(&mut self, kind: LoadKind, data: &[u8]) -> Result<(), String> {
        match kind {
            LoadKind::Rom => self.load_rom(data),
            LoadKind::Bootloader => { S::check_image(data).map_err(|e| format!("the bootloader {e}"))?; self.write_flash(S::BOOTLOADER_OFFSET, data) }
            LoadKind::Flash => { check_flash_image::<S>(data).map_err(|e| format!("the flash image {e}"))?; self.write_flash(0, data) }
            LoadKind::Partitions => self.write_flash(0x8000, data),
            LoadKind::App => { S::check_image(data).map_err(|e| format!("the app {e}"))?; self.write_flash(0x10000, data) }
            LoadKind::Symbols => self.add_symbols(data),
            LoadKind::Script => {
                let text = std::str::from_utf8(data).map_err(|_| "script is not UTF-8")?;
                self.load_script(text)
            }
            LoadKind::CameraPicture if data.starts_with(&[0xff, 0xd8]) => {
                let jpeg = crate::picture::Jpeg::parse(data.to_vec()).map_err(|e| e.to_string())?;
                self.bus.board().set_camera_jpeg(jpeg).map_err(|e| e.to_string())
            }
            LoadKind::CameraPicture => {
                let picture = crate::picture::parse(data)?;
                self.bus.board().set_camera_picture(picture).map_err(str::to_owned)?;
                Ok(())
            }
            LoadKind::SdCard => self.bus.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(data.to_vec())), Default::default()).map(|_| ()),
        }
    }

    /// Resolve a symbol first, then a hexadecimal address with a required `0x` prefix.
    pub fn resolve_stub(&self, name: &str) -> Option<u32> {
        self.sym_addr(name).or_else(|| {
            u32::from_str_radix(name.strip_prefix("0x")?, 16).ok()
        })
    }

    /// Trace every symbol with this prefix, or one exact name when suffixed with `$`.
    pub fn trace_fns(&mut self, pattern: &str) -> usize {
        let exact = pattern.strip_suffix('$');
        let mut matched = 0;
        for (&addr, name) in &self.symbols {
            if exact.map_or_else(|| name.starts_with(pattern), |exact| name == exact) {
                self.fn_probes.insert(addr, name.clone());
                matched += 1;
            }
        }
        matched
    }
}
