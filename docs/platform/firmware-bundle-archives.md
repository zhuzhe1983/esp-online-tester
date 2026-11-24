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

Bounds are 256 MiB compressed, 16,384 entries, a 16 MiB directory and 1 GiB
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

The compressed envelope was increased from 128 to 256 MiB after inspecting the
official C6 LCD 1.69 source archive (142,983,431 bytes, 5,926 entries, 287,190,878
declared bytes). Its exact `Firmware/01_factory.bin` member is 1,297,760 bytes.
The directory and legacy 64 MiB single-BIN limits did not change. The S3 LCD 3.16 source ZIP (214,706,718 bytes, 8,852 entries) declares 717,568,656 bytes in total, so the explicit bundle declaration cap is 1 GiB. Only its pinned 5,704,848-byte BIN is inflated.
Legacy non-UTF-8 names in unselected source documents are validated and compared as exact bytes; their ASCII separators remain subject to traversal checks. A selected path must be ASCII or carry the UTF-8 flag. No legacy filename is written to a filesystem.

The new size does not establish the selected file's factory purpose; its official
documentation and the per-product role are reviewed independently.

Factory status is decided separately from the filename: an official demo image
is not automatically a factory image. Vendor ZIPs and extracted firmware remain
remote references; they are not redistributed with the project or patch site.

The public product and firmware catalog envelopes are now bounded at 8 MiB each
to accommodate independently sourced SKU/revision profiles. Portable scenes
retain their 64 KiB limit and individual private accessory imports retain 16 KiB.
