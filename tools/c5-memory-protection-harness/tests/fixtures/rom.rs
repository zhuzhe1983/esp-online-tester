// Static CSR requests from the pinned rev100 ROM ELF 49693d74... .text listing.
// These are facts about instructions/parameters, not a trace of guest execution.
#[derive(Clone, Copy)]
pub struct Pair {
    pub index: u32, pub address: u32, pub cfg: u32,
    pub address_pc: u32, pub address_word: u32, pub cfg_pc: u32, pub cfg_word: u32,
}
pub const INIT: [Pair; 4] = [
    Pair { index:12,address:0x140007ff,cfg:0xc0000019,address_pc:0x4003ab62,address_word:0xbdc79073,cfg_pc:0x4003ab6e,cfg_word:0xbcc49073 },
    Pair { index:13,address:0x1000ffff,cfg:0xc0000015,address_pc:0x4003abb0,address_word:0xbdd79073,cfg_pc:0x4003abb8,cfg_word:0xbcd79073 },
    Pair { index:14,address:0x1020ffff,cfg:0xc0000019,address_pc:0x4003abfa,address_word:0xbde79073,cfg_pc:0x4003abfe,cfg_word:0xbce49073 },
    Pair { index:15,address:0x10bfffff,cfg:0xc0000011,address_pc:0x4003ac40,address_word:0xbdf79073,cfg_pc:0x4003ac46,cfg_word:0xbcf41073 },
];
pub const FLASH_BOOT: [Pair; 2] = [
    Pair { index:14,address:0x1020ffff,cfg:0xc000001d,address_pc:0x4003ac92,address_word:0xbde79073,cfg_pc:0x4003ac9e,cfg_word:0xbce79073 },
    Pair { index:15,address:0x10bfffff,cfg:0xc0000015,address_pc:0x4003ace0,address_word:0xbdf79073,cfg_pc:0x4003ace6,cfg_word:0xbcf41073 },
];
pub const DIRECT_BOOT: [Pair; 1] = [
    Pair { index:15,address:0x10bfffff,cfg:0xc0000015,address_pc:0x4003ad2c,address_word:0xbdf79073,cfg_pc:0x4003ad36,cfg_word:0xbcf79073 },
];
pub const RAM_BOOT: [Pair; 2] = [
    Pair { index:12,address:0x140007ff,cfg:0xc000001d,address_pc:0x4003ad44,address_word:0xbdc79073,cfg_pc:0x4003ad4e,cfg_word:0xbcc79073 },
    Pair { index:14,address:0x1020ffff,cfg:0xc000001d,address_pc:0x4003ad58,address_word:0xbde71073,cfg_pc:0x4003ad5c,cfg_word:0xbce79073 },
];
pub const DEEP_SLEEP_X: [Pair; 1] = [
    Pair { index:12,address:0x140007ff,cfg:0xc000001d,address_pc:0x4003ad6e,address_word:0xbdc79073,cfg_pc:0x4003ad78,cfg_word:0xbcc79073 },
];
