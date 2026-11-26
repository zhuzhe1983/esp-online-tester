# Five historical M5Stack SKUs

Baseline: `4dabba0c`. All five listings are EOL. They remain separate from their replacement products and hardware revisions.

| SKU | Exact identity and new preview | Runtime boundary |
| --- | --- | --- |
| K016-B | Original M5Stick, ESP32-WROOM32, SH1107 64×128 OLED; independent photo-derived25.5×48.2×13.7mm enclosure | Independent SH1107 SPI/page-RAM/button35 runtime added; no StickC ST7735/AXP192 identity is inherited. |
| K016-T | Original M5StickT, FLIR Lepton3.0,135×240 display,300mAh; independent photo envelope with rear optical lens and open mounting-ear holes | Original manufacturer Markdown recovered:4MB Flash,ST7789,MPU6886,AXP192 and48×30×29mm. Independent original LCD/key/Lepton digital runtime added; encoder MCU and physical calibration remain unimplemented. |
| K022 | M5GO Lite package, old21mm-thick M5GO host and one ENV unit | Existing Classic LCD/three-button/SPI-SD subset only; ENV and other package accessories are not automatically instantiated or verified. |
| K006 | Old M5GO IoT Starter Kit with six accessory Units;21mm host preview | Same source-confirmed LCD/key/SD nets use the existing partial Classic implementation. No factory firmware execution is inherited. |
| K007 | Old FIRE PSRAM2.0 listing,28.6mm host/charging-base preview | Partial Classic LCD/key/SD subset. Current table says8MB PSRAM while same-SKU historical photos say4MB; the selected8MB is explicitly a simulation configuration. |

The three Core previews retain the existing K001 manufacturer top vertices, including its independent LCD/three-key surfaces, with only a common Z translation. This is related family CAD, not a claim that the K001 file is exact K006/K007/K022 assembly CAD. Separate lower bases are constrained by the old-SKU dimensions and photos. Bottom LEGO holes, magnets, hidden latches, pogo contacts, charging-tray attachment, optical materials and full photo alignment remain unverified. The21mm M5GO hosts are not stretched versions of28.6mm v2.7 models.

The original K016-T manufacturer Markdown now supplies the30×48×29mm overall envelope. Its independent photo approximation is rebuilt from that envelope; main-shell27.87mm depth is still estimated by subtracting the photo-derived button/lens projections. No manufacturer assembly is scaled. The former24mm shell/25.2mm envelope estimate is superseded. See [current runtime and model evidence](m5-classic-completion-runtime-20261126.md). Neither Stick preview invents USB/Grove mating geometry when the photographs do not establish its mouth/depth. Product interface facts remain separately available.

Every preview retains an UNKNOWN board and source-declared inspection offsets. Screens use the runtime framebuffer surface, not photographs of the factory application. Source photos retain M5Stack copyright. Mathematical approximation geometry and related manufacturer geometry have separate notices.

Factory investigation found exact old M5Stick and FIRE recovery-tool references but no version-pinned complete browser BIN in the fetched sources. K006/K022/K016-T remain program/source references. The M5GO-Lite page's cross-SKU Fire recovery link is explicitly rejected. No firmware binary was downloaded, mirrored, substituted or marked compatible. Exact-SKU factory/UIFlow/Xiaozhi investigations are manual watchlist records until a unique version/image mapping is established.

Validation loads all five actual models, checks front-key ray hits and material/inspection/reset behavior, rebuilds all five models offline byte-for-byte, and compares every reused K001 vertex to prove the only coordinate change is its declared translation. This does not establish physical fit, full1:1 photograph agreement, original firmware boot or complete device compatibility.

Rebuild one preview from committed inputs:

```sh
python tools/build-m5-legacy-kit-models.py m5stack-k006 --output /tmp/m5go-rebuilt
node --test tools/platform-m5-legacy-kit-model-test.mjs
```

Sources, firmware findings and final hashes: [evidence](evidence/m5stack-legacy-kits-20261126/).
