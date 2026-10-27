# Portable scenes and catalogs

The executable helpers are `web/platform/catalog.js`; structural contracts are in `web/platform/schemas/`. Schemas document structure; `validateScene` also resolves references and hardware constraints. It returns `{valid,errors,warnings}`, with issues `{code,path,message}`.

```js
{
  schemaVersion: '1', productId: 'm5stack-stopwatch',
  firmware: {manifestUrl: 'https://example.org/fw/manifest.json', sha256: '64 hexadecimal digits'},
  accessories: [{id:'gps-1',catalogId:'m5stack-gps-u032',parameters:{},
    wiring:{mode:'logical',port:'uart1'}}],
  keyMapping: {KeyA:'button-a',KeyB:'button-b'},
  view: {mode:'studio',deviceView:'3d'},
  localAccessories: []
}
```

`firmware:null` is valid for designing a scene; execution needs a source. Exactly one of `manifestUrl` or raw image `url` is allowed. A raw remote source may include `bootMode:'rom'|'app-direct'`; when omitted, a merged image is preferred, then a single ESP application is accepted only after chip, descriptor, segment, checksum and optional embedded SHA-256 validation. The selected mode is saved in the 40-entry browser history. Application-direct does not create a bootloader or partition table and cannot be flashed to physical hardware as a complete image. URLs are absolute HTTPS; loopback HTTP is accepted for local development. SHA-256 is optional before loading but should be pinned for reproducible work. For a manifest, source SHA-256 pins the manifest bytes, not mutable files referenced inside it. The platform accepts `{url,sha256}` artifact objects. For compatibility with upstream string-based manifests, retain string file paths and add `artifact_sha256:{'public/app.bin':'64 hexadecimal digits'}`; this applies to `files` and `flash_at` entries. Retain the runtime's computed artifact receipt. The repository-only StopWatch test fixture pins its manifest and every referenced artifact; it is excluded from the public Pages build.

`encodeScene(scene)` uses canonical sorted-key JSON and UTF-8 base64url. `decodeScene(encoded)` rejects malformed, oversized or unsafe payloads but does not resolve catalog references; call `validateScene` next. The scene limit is 64 KiB, with at most 32 accessory instances and 32 embedded local definitions. URL encoding is not encryption: never put credentials, private signed URLs or personal data in a shared scene. Read the user intent before deciding what to share.

Physical UART declarations use `{mode:'physical',port:'uart1',connectorId:'grove',rx:11,tx:10}`. Pins must belong to a verified external connector and may not conflict. This describes a proposed wiring only; the current runtime rejects physical injection with `UNSUPPORTED_WIRING`. StopWatch's Grove has G10/G11, but the controller and direction depend on firmware. PaperMono has no verified external GPS connector in this catalog. Never repurpose its internal buses as a guessed expansion connector.

## Personal DIY accessories

Create declarative definitions with:

```js
const definition = createLocalAccessory({
  id:'local:my-sensor', name:'My sensor', description:'Personal design',
  modelId:'metadata-only',
  connector:{type:'custom',protocol:'i2c',voltage:3.3,
    pins:[{name:'SDA',direction:'bidirectional'},{name:'VCC',direction:'power'}]},
  parameters:{}
});
saveLocalAccessory(definition);
```

Model choices are `metadata-only` (no behavior) and `uart-nmea` (logical GPS input only). Pin directions are `input`, `output`, `bidirectional`, `power`, `ground`; protocols are `uart`, `i2c`, `gpio`, `spi`. Arbitrary JS plugins are not supported. `listLocalAccessories`, `saveLocalAccessory`, and `removeLocalAccessory` use this site's browser localStorage. They never submit entries to the public library. Corrupt storage raises an error rather than silently discarding the user's designs.

`exportLocalAccessory(definition)` returns JSON; `importLocalAccessory(json)` validates and returns the definition without saving it. To make a DIY scene portable, copy referenced definitions into `scene.localAccessories`; another browser cannot read the sender's private localStorage. Definitions use `local:` IDs to avoid collisions with public IDs. Each is limited to 16 KiB; storage holds at most 32.

## Public contributions

`view.appearance` optionally stores `{shell:{color:'#d7ab52',opacity:1,finish:'metal'}}`. These are viewer choices, not product facts. All three shell fields are required when present; unknown fields, nonfinite/out-of-range opacity and non-hex colors are rejected. Existing scenes without `appearance` retain their product's original shell. Update it with `updateSceneMetadata` to avoid resetting execution. Share links and the iframe bridge use the same scene contract.

Finish IDs are `plastic`, `glossy-plastic`, `frosted-plastic`, `clear-resin`, `rubber`, `metal`, `brushed-metal`, and `ceramic`; `appearance.js` exports their display labels. Firmware entries link to version tracking with `watchlistId`, while `productIds` controls device compatibility filtering. The user-facing firmware library is the single selection surface; a watchlist entry alone does not establish device support.

Edit the appropriate `web/platform/catalog/{products,accessories,firmwares}.json` through a PR. Include stable IDs, hardware SKU/revision, model status, limitations and primary source URLs with `checkedAt`. Keep physical metadata separate from implemented runtime support. Mark unresolved firmware projects `status:'reference',source:null`; only use `available` when a concrete loadable source exists. An `available` entry still needs actual runtime assertions before a compatibility claim.

New products require an implemented board model for execution; JSON and a 3D shell alone do not add peripheral behavior. New GPS variants retain separate baud defaults. Keep visible screen geometry and emulator framebuffer geometry distinct. Do not claim silicon validation from emulator-only results.
