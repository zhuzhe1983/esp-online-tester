# Original recovery BINs in manufacturer source ZIPs

Some Waveshare recovery images are distributed inside large official source/demo
archives. A flat single-BIN ZIP descriptor cannot represent those downloads.
An explicit `zip-bundle` descriptor now identifies the original container by URL
and SHA-256, and selects exactly one relative `.bin` path with its own SHA-256:

```json
{
  "url": "https://vendor.example/device-demo.zip",
  "sha256": "<64 hexadecimal digits for the selected BIN>",
  "archive": {
    "format": "zip-bundle",
    "entry": "device-demo/firmware/factory.bin",
    "sha256": "<64 hexadecimal digits for the untouched ZIP>"
  }
}
```

The runtime fetches the original ZIP and checks the container hash. The decoder
checks every central-directory/local-header range and name, then decompresses
only the named member. The existing single-BIN decoder checks the selected CRC;
the bundle decoder checks the resulting BIN hash. The selected image still goes
through the ordinary chip, capacity, partition, boot and firmware-target gates.
The descriptor survives scene sharing and the canonical image cache. Firmware
matching includes the container hash as well as the member path and BIN hash.

Bounds are 128 MiB compressed, 16,384 entries, a 16 MiB directory and 512 MiB
declared total expansion. Unselected entries are never inflated or written to
disk. Absolute/traversing paths, duplicate names, links, encryption, unsupported
compression, overlapping entries, undeclared data, split archives and ZIP64 are
rejected. The selected BIN remains limited by its actual device Flash capacity.
Legacy single-BIN ZIP and user import limits remain unchanged. Physical flashing
retains its existing requirement to select the complete extracted BIN.

Verification is retained in
`evidence/waveshare-p4-batch008-20261124/vendor-bundle-reader.json`: the untouched
official C6 LCD 1.3 ZIP (97,520,767 bytes), Thermal demo ZIP (69,229,150 bytes) and
Thermal web demo ZIP (65,281,239 bytes) all select the pinned member correctly.
Unit tests cover malformed ranges, paths, CRC, hashes, cancellation and expansion
bounds; the production Worker test exercises loading, starting, sharing, cache
reuse and both hash-mismatch paths. These checks establish archive transport and
image identity. They do not establish complete original application support.

Factory status is decided separately from the filename: an official demo image
is not automatically a factory image. Vendor ZIPs and extracted firmware remain
remote references; they are not redistributed with the project or patch site.

The public product and firmware catalog envelopes are now bounded at 8 MiB each
to accommodate independently sourced SKU/revision profiles. Portable scenes
retain their 64 KiB limit and individual private accessory imports retain 16 KiB.
