# Classic ESP32-CAM: host JPEG through I2S0 camera DMA (EX256)

Two original commits on main `66eb9704`, followed by the padding review fix:

1. `esp32: the AI-Thinker camera's SCCB answers on either I2C controller`:
   the OV2640 on GPIO26/27 was attached to I2C0 only.
2. `esp32, esp-soc: host JPEG for the classic ESP32-CAM`: the subject of EX256.

## Contract

The host supplies already-encoded baseline JPEG; the emulator has no encoder
and never decodes, re-encodes or scales. `esp_soc::picture::Jpeg::parse`
checks marker and segment structure only: SOI first; SOF0, DHT, DQT, DRI,
SOS, APP0–APP15, COM and EOI are the only allowed markers; DNL is refused.
Exactly one SOF0 must have length 8 + 3*Nf, 1–4 distinct components,
8-bit precision, nonzero dimensions, sampling factors 1–4 and quantization
selectors 0–3. Each SOS after SOF0 has length 6 + 2*Ns, 1–Nf distinct known
components, DC/AC selectors 0–1, Ss=0, Se=63 and Ah=Al=0. Each scan must
contain entropy data; stuffed bytes and restart markers are walked. DRI has
length 4; DHT, DQT, APPn and COM are length-bounded only. EOI must end input.
This does not decode the image or validate table contents or semantics.
Other SOF types are `NotBaseline`; more than 8 MiB is
`TooLarge`. `HostFrameSource` keeps the JPEG next to (not instead of) the raw
picture, so raw RGB565/YUV422 output, its conversion cache and the preview are
unchanged by JPEG input.

The AI-Thinker board accepts a JPEG whose SOF0 size equals the OV2640's
configured output size and refuses any other with
`SizeMismatch { jpeg, sensor }`, keeping the previous input. Before the guest
configures the sensor (a zero dimension) the size is checked at delivery
instead. JPEG output is IMAGE_MODE 0x10 (JPEG_EN), optionally with 0x02
(HREF_VSYNC; esp32-camera's `ov2640_settings_jpeg3` writes 0x12); in that mode
only a host JPEG of exactly the configured size is a frame. Without one, the
mode delivers no frame and prints one notice.

At a frame start in JPEG mode the I2S0 capture packs the JPEG bytes through the
same FIFO packer as raw rows, then appends zero words up to one word before an
RXEOF_NUM boundary, never ending inside the first EOF block when RXEOF_NUM is
at most 8 MiB in words. Larger counts are delivered without padding; the existing
DMA rejects counts whose conversion to bytes overflows u32. esp32-camera
v2.1.4 (`cam_hal.c` 291-380) checks SOI on the first EOF block, copies the
unfinished block at VSYNC and searches EOI backwards (`cam_hal.c` 755), so the
framebuffer holds the host bytes, `fb->len` is the host length and no stale DMA
bytes follow EOI. (Line numbers: esp32-camera v2.1.4, `b817e9b8`,
`driver/cam_hal.c`.) The zero padding after EOI is a model choice (inferred), not a
silicon or OV2640 timing claim. Delivery is the existing uniform approximation
over the fixed ~10 Hz frame, from the falling edge as for raw frames. Raw
frames claim the frame-start opportunity when the frame is built; JPEG sets an
independent claim flag on the capture and consumes it with its first delivered
word, claiming only while that edge is still the pending one. The flag is cleared
when a frame ends, and stopping or resetting RX discards its capture state
(a single long bus step across the next VSYNC pulse leaves the newer
edge to the next frame, as for raw frames). An RX stop/reset/start inside the
existing arm window (`ARM_WINDOW`) before that word therefore takes the whole
JPEG frame from SOI, and a frame whose first word was delivered is never
replayed. There is no back-porch delay: JPEG words start at the falling edge
over the same span as raw rows. This is a model choice for a restart that comes
after the falling edge, not a timing claim.

API added (public):

```rust
// esp-soc/src/picture.rs
pub struct Jpeg { pub w: u32, pub h: u32, pub bytes: std::sync::Arc<Vec<u8>> }
pub enum JpegError { Malformed, NotBaseline, TooLarge, SizeMismatch { jpeg: (u32, u32), sensor: (u32, u32) }, Unsupported }
impl Jpeg { pub fn parse(bytes: Vec<u8>) -> Result<Self, JpegError> }
// esp-soc/src/camera.rs
impl HostFrameSource {
    pub fn set_camera_jpeg(&mut self, jpeg: picture::Jpeg, sensor: (u32, u32)) -> Result<(), picture::JpegError>;
    pub fn jpeg_frame(&self, w: u32, h: u32) -> Option<(u32, u32, std::sync::Arc<Vec<u8>>)>;
}
// esp-soc/src/board.rs, BoardModel (defaults: Err(Unsupported) / false)
fn set_camera_jpeg(&mut self, _jpeg: crate::picture::Jpeg) -> Result<(), crate::picture::JpegError>;
fn camera_frame_is_jpeg(&self) -> bool;
```

Input paths: CLI `--cam-image` and loader kind 7 (`Machine::load`, used by the
browser) treat data starting with FF D8 as JPEG. Only the AI-Thinker board
overrides the two trait methods; every other board keeps the defaults. A JPEG
file given to another board's camera now exits 2 with `[emu] <path>: jpeg: this
board's camera does not accept JPEG input` where main said
`[emu] <path>: unsupported image`.

## Guest fixture

The compatibility fixture `browser-420.jpg` is a self-made 2×2 RGB image,
encoded with libjpeg-turbo `cjpeg` 3.2.0 (`-baseline -sample 2x2 -quality 75`).
Its PPM input is `P6\n2 2\n255\n` followed by RGB bytes for red, green, blue,
and white, in that order. The JFIF image has three components, 4:2:0 sampling,
669 bytes, SHA-256 `7916d4fb61725d036804562db0fbb3e7d3c8fd46fa92f171fd1714c1f9b3b5ee`.

[Recipe](../../../examples/classic-camera/jpeg.md). Pinned ESP-IDF v5.4.3
`ea1c174c`, esp32-camera `b817e9b8`, `PIXFORMAT_JPEG`, 20 MHz XCLK, quality 12.
Two clean builds per variant in different source/output directories are byte
identical and equal the committed images:

| image | bytes | SHA-256 |
| --- | ---: | --- |
| `jpeg.bin` (1 framebuffer, DRAM) | 248304 | `f29370065e3966fb53164e183650a69398094ce0e4e635a183d369d06c797f6d` |
| `jpeg-psram.bin` (2 framebuffers, PSRAM, 4 MiB) | 266048 | `9753fb3d05febd8407263821b0c4361dc9d87e431a04226e44096054bb7a4079` |
| `bootloader/bootloader.bin` (shared) | 25584 | `9fe22a0bd805d8830602afa2d92f99312b7f567503db22808e3f7cc903f68850` |
| `partition_table/partition-table.bin` (shared) | 3072 | `7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820` |

`esp32/tests/camera_jpeg.rs`, both variants, ROM boot: every line below appears
exactly once (DRAM / PSRAM per variant), plus three queued frames that must each
be the old or the new JPEG.

| phase | frames | size | len | SHA-256 |
| --- | --- | --- | ---: | --- |
| vga-a | 0, 1 | 640 × 480 | 6218 | `f602a493…a576998` |
| vga-b (host change, camera running) | 0, 1 | 640 × 480 | 11400 | `680228c7…720cde4` |
| qvga-a (`set_framesize(QVGA)`, host change) | 0, 1 | 320 × 240 | 2489 | `7a7f480a…f8932d0` |
| qvga-b (host change) | 0, 1 | 320 × 240 | 2487 | `a711e431…dec968` |
| raw (deinit, RGB565 QQVGA, 1 framebuffer) | 0, 1 | 160 × 120 | 38400 | `c67913e0…aed6cf1` (C8 oracle) |
| reinit (deinit, JPEG QVGA) | 0, 1 | 320 × 240 | 2487 | `a711e431…dec968` |

After setting `vga-a.jpg`, an SOF0/EOI input with no scan is refused; the first
two guest frames still have the original VGA length and hash above. At the QVGA marker the host's `vga-a.jpg` is refused with
`SizeMismatch { jpeg: (640, 480), sensor: (320, 240) }` and a truncated
`qvga-a.jpg` with `Malformed`; the next frames are still the previous input
until `qvga-a.jpg` is accepted. The CLI test runs `--cam-image vga-a.jpg` with
`jpeg.bin` and pins the first two VGA frames; a truncated file exits 2 with
`[emu] <path>: jpeg: malformed ...` before the machine starts.

## Arduino sketch

[`arduino-probe.ino`](arduino-probe.ino): a common ESP32-CAM Arduino camera
setup (AI-Thinker pins, 20 MHz XCLK, `PIXFORMAT_JPEG`, `FRAMESIZE_VGA`,
`jpeg_quality = 12`, `fb_count = 1`; `camera_config_t` on the stack with
`fb_location` and `grab_mode` never set; `sccb_i2c_port` is also unset but
unused because SDA/SCL pins are supplied), with the network
code replaced by a hash of each frame. Built with arduino-cli 1.5.1, core
esp32:esp32 3.3.11, FQBN `esp32:esp32:esp32cam`; merged image
`48f5039f08b78ea5c72a9cf8cb43ba45d82de38e390d60aa2a677045e2ab6b67`.

```sh
esp32sim --chip esp32 --board ai-thinker-cam --psram-mb 4 --boot rom \
  --flash-image probe.ino.merged.bin --cam-image vga-a.jpg --max-seconds 3
```

| build | result |
| --- | --- |
| main `66eb9704` | `Detected camera not supported`, `S init=262`; `--cam-image vga-a.jpg` is `unsupported image` |
| this branch | `S init=0 psram=1`, frames 0..8 each `640x480 len=6218 format=4 PSRAM sha256=f602a493…a576998` |
| this branch, `--psram-mb 0` | `frame buffer malloc failed` |

With nothing set, the stack left `fb_location` as `CAMERA_FB_IN_PSRAM` in
this run (inferred from the allocation failure; an uninitialized value is not
a contract). With explicit SDA/SCL pins, the driver calls `SCCB_Init`, which
uses `CONFIG_SCCB_HARDWARE_I2C_PORT` (default I2C1), not `sccb_i2c_port`.
The C8 fixture selects PORT0 in its `sdkconfig.defaults`. The sketch needs the
optional 4 MiB PSRAM; the classic default (none) is unchanged here.

## Checks and mutations

With Rust 1.99.0, native and wasm clippy, ROM-enabled and plain workspace
tests, the wasm build and eight demo tests passed. The privacy check passed.
Seven normalized fixture outputs, exit codes and instruction counts matched a
fresh build of main `66eb9704` on the earlier `628206d6` head; 21 selected
normalized hot-path symbols matched that base. Both builds used `cargo +1.99.0
build --release --bins` with the same profile.

Padding regression: RXEOF_NUM `0x0400_0000` leaves the packed JPEG unpadded;
`u32::MAX` triggers the existing DMA byte-count overflow error without an EOF.
The test process covering both used 8,323,072 bytes maximum RSS on the native
host. The unit test also checks the inclusive 8 MiB-in-words limit, one word
above it, and checked addition at `usize::MAX`. Removing the limit and making
it exclusive each fail an assertion; replacing checked addition with unchecked
arithmetic panics in the debug test. This is a panic-detected overflow mutant,
not an assertion kill. The `examples/classic-camera/mutate.py` public-driver
control passed and its five transport mutants were killed by assertions.

A campaign on `628206d6` passed all five unmutated control plans and killed 88
of 93 mutants by assertion. Five survivors are
redundant guards: P6 is rejected on the next marker; P9 and S5 cannot produce
a valid scan without frame components; S14 cannot select more distinct known
components than exist; S15 is enforced by the final three-byte SOS parameter
slice comparison. The campaign does not prove image decoding or semantics.

| Mutation | Result / failing test |
| --- | --- |
| P1 no SOI check | `picture::tests::jpeg_structure_and_sof0_size` |
| P3 EOI need not end input | `picture::tests::jpeg_structure_and_sof0_size` |
| N1 scan flag never set | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| N2 EOI ignores scan | `picture::tests::jpeg_requires_scan_after_sof0` |
| N3 refusal overwrites previous frame | `public_driver_jpeg_psram_two_framebuffers`, `public_driver_jpeg_dram_single_framebuffer` |
| P4 progressive etc. accepted | `picture::tests::jpeg_structure_and_sof0_size` |
| P5 no fill bytes | `picture::tests::jpeg_structure_and_sof0_size` |
| P6 segment length < 2 allowed | Survived: redundant guard (see above) |
| P7 second SOF0 allowed | `picture::tests::jpeg_structure_and_sof0_size` |
| P8 any precision | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| P9 SOS before SOF0 | Survived: redundant guard (see above) |
| P10 RST ends the scan | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| P11 no size limit | `picture::tests::jpeg_structure_and_sof0_size` |
| P12 width/height swapped | `picture::tests::jpeg_structure_and_sof0_size` |
| P13 restart marker outside scan accepted | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| C1 no set-time size check | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| C2 unconfigured sensor refuses | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| C3 no delivery size filter | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| C4 JPEG input drops raw cache | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| C5 JPEG copied, not shared | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| A1 0x12 not JPEG | `aithinker_unconfigured_sensor_accepts_jpeg_checked_at_delivery`, `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size`, `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing` |
| A2 JPEG+RGB565 taken as JPEG | `unsupported_sensor_output_warns_once` |
| A3 board skips size check | `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size` |
| A4 capture never sees JPEG mode | `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size`, `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing`, `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| A5 JPEG notice every frame | `unsupported_sensor_output_warns_once` |
| A6 SCCB on I2C0 only | `aithinker_selection_and_only_sensor_attachment`, `aithinker_sccb_is_one_sensor_on_either_controller` |
| A7 SCCB register file per controller | `aithinker_sccb_is_one_sensor_on_either_controller` |
| A8 SCCB ignores power | `aithinker_pwdn_stop_and_reset_withhold_frames`, `aithinker_sccb_is_one_sensor_on_either_controller` |
| D1 no zero padding | `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| D2 padding ends on the EOF boundary | `camera_dma::tests::jpeg_padding_ends_one_word_before_an_eof_boundary_after_one_block` |
| D3 may end inside the first EOF block | `camera_dma::tests::jpeg_padding_ends_one_word_before_an_eof_boundary_after_one_block` |
| D4 first word claims any pending edge | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| D5 JPEG never claims the frame | `c7::arming::jpeg_claims_the_frame_with_its_first_word` |
| D6 JPEG claims at frame start | `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::arming::first_word_claims_only_its_own_frame_edge` |
| D12 first word claims whenever an edge is pending | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| D13 claim on an empty delivery | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| F1 flag never set | `c7::arming::jpeg_claims_the_frame_with_its_first_word` |
| F2 flag never consumed | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| F3 guard ignores flag | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| F4 guard ignores newer edge | `camera_dma::tests::jpeg_claim_state_is_consumed_once_and_never_claims_a_newer_edge` |
| D7 JPEG packed as raw rows | `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing` |
| D8 no partial-word flush | `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| I1 RXEOF_NUM from the wrong register | `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| L1 loader never takes JPEG | `loader_kind_7_takes_jpeg_with_typed_errors`, `public_driver_jpeg_dram_single_framebuffer`, `public_driver_jpeg_psram_two_framebuffers` |
| CL1 CLI never takes JPEG | `classic_camera_jpeg_file_reaches_framebuffer_verbatim` |
| CL2 CLI JPEG error without the [emu] prefix | `classic_camera_jpeg_file_reaches_framebuffer_verbatim` |
| D10 JPEG first word delayed by ARM_WINDOW | `c7::arming::jpeg_claims_the_frame_with_its_first_word` |
| D11 JPEG first word delayed by ARM_WINDOW / 2 | `c7::arming::jpeg_claims_the_frame_with_its_first_word` |
| R1 second SOI is a segment | `picture::tests::jpeg_structure_and_sof0_size`, `picture::tests::jpeg_segment_validation` |
| R2 SOF0 minimum length 6 | `picture::tests::jpeg_segment_validation` |
| R3 zero height accepted | `picture::tests::jpeg_structure_and_sof0_size` |
| R4 SOF1 accepted as baseline | `picture::tests::jpeg_structure_and_sof0_size` |
| R5 stuffed byte ends the scan | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| R6 size limit off by one (>=) | `picture::tests::jpeg_structure_and_sof0_size` |
| R7 set-time check needs only one configured dimension | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| R8 refusal still replaces the stored JPEG | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| R9 delivery filter checks width only | `camera::tests::host_jpeg_is_separate_verbatim_and_size_checked` |
| R10 only 0x12 is JPEG mode | `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size` |
| R11 board passes sensor size swapped | `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size` |
| R12 JPEG mode without host JPEG falls back to raw RGB565 | `aithinker_host_and_register_updates_invalidate_shared_cache`, `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size`, `aithinker_unconfigured_sensor_accepts_jpeg_checked_at_delivery`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing`, `unsupported_sensor_output_warns_once` |
| R13 capture adapter never told the frame is JPEG | `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size`, `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing` |
| R15 padding ends on the EOF boundary (-1 dropped) | `camera_dma::tests::jpeg_padding_ends_one_word_before_an_eof_boundary_after_one_block` |
| R16 no first-block minimum | `camera_dma::tests::jpeg_padding_ends_one_word_before_an_eof_boundary_after_one_block` |
| R17 JPEG frame claimed at build time | `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::arming::jpeg_claims_the_frame_with_its_first_word` |
| R20 padding with 0xff instead of zeros | `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| R21 RXEOF_NUM read as bytes not words | `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof` |
| R22 SCCB only on I2C1 | `aithinker_selection_and_only_sensor_attachment`, `aithinker_host_and_register_updates_invalidate_shared_cache`, `aithinker_pwdn_stop_and_reset_withhold_frames`, `aithinker_sccb_is_one_sensor_on_either_controller`, `aithinker_grayscale_requests_keep_yuv_wire_bytes`, `aithinker_xclk_rejects_constant_or_invalid_clock`, `aithinker_jpeg_output_is_host_jpeg_verbatim_at_configured_size`, `aithinker_unconfigured_sensor_accepts_jpeg_checked_at_delivery`, `aithinker_sccb_requires_both_physical_wires`, `c6_receiver_unarmed_reset_and_modes_withhold_routed_frames`, `aithinker_xclk_requires_live_routed_pwm_not_gpio_level`, `c6_vsync_unarmed_stopped_irq_and_one_us_level_without_dma`, `c6_deadline_edges_preserve_timestamps_and_large_steps`, `c7::arming::c71_driver_stop_reset_start_inside_window`, `c6_power_clock_and_sensor_stop_cancel_and_restart_vsync`, `c6_physical_routes_inversion_and_reconfiguration`, `c7::arming::c71_falling_edge_full_frame_and_tick_splits`, `c7::arming::c71_inclusive_window_boundary_and_exact_catchup`, `c7::arming::c71_forced_low_power_clock_stream_stop_never_starts_frame`, `c7::arming::c71_initially_invalid_route_waits_instead_of_aborting`, `c7::arming::c71_route_loss_and_restore_inside_window_requires_new_pulse`, `c7::arming::c71_unrelated_pad_and_noroute_bit_writes_keep_window`, `c7::arming::first_word_claims_only_its_own_frame_edge`, `c7::arming::c71_second_start_cannot_replay_claimed_frame`, `c7::c7_bad_owner_forbidden_descriptor_and_buffer_never_eof`, `c7::c7_dport_reset_drops_capture_and_requires_fresh_inlink`, `c7::arming::jpeg_claims_the_frame_with_its_first_word`, `c7::c7_inlink_rmw_stop_must_remain_stopped_until_fresh_start`, `c7::c7_exact_native_descriptors_two_eofs_and_classic_irq`, `c7::c7_qqvga_pattern_twenty_descriptors_ten_serviced_eof_chunks`, `c7::c7_reacquire_next_image_on_reused_ring`, `c7::c7_mid_frame_route_or_power_loss_is_terminal_until_rearm`, `c7::c7_ring_starvation_and_finite_guest_service_reuse`, `c7::c7_stop_restart_and_every_rx_reset_discard_incomplete_frame`, `c7::c7_rx_disable_rmw_stop_and_reenable_requires_fresh_start`, `c7::c7_tick_split_invariance_and_late_arm_waits_vsync`, `c7::cancellation::cancellation_iomux`, `c7::cancellation::cancellation_i2s_gate`, `c7::cancellation::cancellation_ledc_gate`, `c7::cancellation::cancellation_matrix`, `c7::cancellation::cancellation_power`, `c7::cancellation::cancellation_qualified_waiting`, `c7::cancellation::cancellation_streaming`, `c7::cancellation::cancellation_rtc`, `c7::cancellation::cancellation_valid_changes_preserve_active_capture`, `c7::cancellation::cancellation_xclk`, `c7::inverted_route_reuses_capture_cache_and_invalidates_source_and_mask`, `c7::jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof`, `c7::jpeg_mode_without_matching_host_jpeg_delivers_nothing`, `unsupported_sensor_output_warns_once` |
| R23 SCCB ignores power on I2C1 (powered check dropped) | `aithinker_sccb_is_one_sensor_on_either_controller`, `aithinker_pwdn_stop_and_reset_withhold_frames` |
| R24 loader JPEG goes to raw parser | `loader_kind_7_takes_jpeg_with_typed_errors`, `public_driver_jpeg_psram_two_framebuffers`, `public_driver_jpeg_dram_single_framebuffer` |
| R25 CLI ignores JPEG refusal | `classic_camera_jpeg_file_reaches_framebuffer_verbatim` |
| R26 EOI need not be last (trailing bytes allowed) | `picture::tests::jpeg_structure_and_sof0_size` |
| R27 JPEG notice re-armed every miss | `unsupported_sensor_output_warns_once` |
| S1 allow DNL | `picture::tests::jpeg_refuses_incomplete_sos_and_dnl`, `picture::tests::jpeg_segment_validation` |
| S2 allow DAC | `picture::tests::jpeg_segment_validation` |
| S3 allow TEM | `picture::tests::jpeg_segment_validation`, `picture::tests::jpeg_structure_and_sof0_size` |
| S4 allow zero marker | `picture::tests::jpeg_segment_validation` |
| S5 SOF count min | Survived: redundant guard (see above) |
| S6 SOF count max | `picture::tests::jpeg_segment_validation` |
| S7 SOF exact length | `picture::tests::jpeg_segment_validation` |
| S8 SOF duplicate component | `picture::tests::jpeg_segment_validation` |
| S9 SOF horizontal sample | `picture::tests::jpeg_segment_validation` |
| S10 SOF vertical sample | `picture::tests::jpeg_segment_validation` |
| S11 SOF quantization | `picture::tests::jpeg_segment_validation` |
| S12 DRI length | `picture::tests::jpeg_segment_validation` |
| S13 SOS count min | `picture::tests::jpeg_segment_validation` |
| S14 SOS count max | Survived: redundant guard (see above) |
| S15 SOS exact length | Survived: redundant guard (see above) |
| S16 SOS known component | `picture::tests::jpeg_segment_validation` |
| S17 SOS distinct components | `picture::tests::jpeg_segment_validation` |
| S18 SOS DC selector | `picture::tests::jpeg_segment_validation` |
| S19 SOS AC selector | `picture::tests::jpeg_segment_validation` |
| S20 SOS spectral parameters | `picture::tests::jpeg_segment_validation` |
| S21 scan entropy | `picture::tests::jpeg_rejects_scan_without_entropy` |

## CPU comparison

Maintainer's [run](https://github.com/mikroverk/esp32sim/pull/328#issuecomment-6540589479),
2026-10-31: user CPU, base `66eb9704`, new `0f54dbc2`. The maintainer
used `docs/evidence/p4-cpu/cpu_pairs.py` from the base with a local
`classic-hello` workload added. Both trees were extracted with `git archive`
into sibling directories of equal path length and built with
`cargo build --release --bins`, rustc 1.99.1 (58cf8f9c3 2026-10-13),
`aarch64-apple-darwin`, Apple M5 Max (18 cores), macOS 27.0.1. ROMs were
esp-rom-elfs 20260528 (`tools/fetch-rom-elfs.sh`). Each workload started at
1-minute load below 5 with nothing building; one warmup pair preceded seven
alternating pairs, run from the base tree's root.

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.250–0.260) | 0.260 s (0.250–0.260) | +0.0% | 1/7 | identical in all 16 runs | 3.48 → 3.36 |
| c3-hello (30 s) | 2.530 s (2.460–2.620) | 2.470 s (2.440–2.530) | -2.4% | 1/7 | identical in all 16 runs | 3.36 → 3.56 |
| c6-hello (30 s) | 1.600 s (1.580–1.650) | 1.620 s (1.570–1.640) | +1.2% | 3/7 | identical in all 16 runs | 3.56 → 3.29 |
| pocket-tank (8 s) | 7.260 s (7.110–7.400) | 7.250 s (7.170–7.440) | -0.1% | 3/7 | identical in all 16 runs | 3.29 → 4.43 |
| classic-hello (30 s) | 0.370 s (0.370–0.380) | 0.380 s (0.360–0.380) | +2.7% | 3/7 | identical in all 16 runs | 4.43 → 4.40 |

The maintainer's verdict: nothing flagged (slower in at least 6/7 pairs is the
flag). Classic-hello is +2.7%, slower in 3/7, with overlapping ranges; the
camera path runs only in camera mode. These are non-camera workloads, not a
measurement of camera-mode performance.

[Paired-run JSON](328-0f54dbc2-66eb9704.json) is the maintainer's posted
sanitized data; only the ROM directory was replaced with `$ESP32SIM_ROM_DIR`.
The maintainer reported original SHA-256 prefix `719dd684` and sanitized
SHA-256 prefix `45865fd8`; the original is not retained here. The JSON code
block as posted has SHA-256
`1b85653e3a8e25f891aa83fa8e33cdc0f629f2812d5d1980d07a7b1e718e7fa0`;
its bytes do not match that reported sanitized-file prefix.

## Limits

No JPEG encoding, decoding, quality or scaling: the host supplies JPEG of the
configured size. Fixed approximate 10 FPS; adjustable FPS unsupported. Raw
capture with two framebuffers is not covered. Not checked on hardware.
