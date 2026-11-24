# C5 catalog fragment generation

`tools/generate-waveshare-c5-entries.py` converts an independent runtime patch,
its verified source manifest and explicit board-specific catalog fields into
reviewable JSON fragments. It writes only the supplied output directory.
It does not edit the shared catalog, copy firmware into the public directory,
register CLI/WASM routes or publish anything.

For the Touch3.5 batch:

```sh
python3 tools/generate-waveshare-c5-entries.py \
  docs/research/waveshare-c5-touch35-central-patch.json \
  --fields docs/research/waveshare-c5-touch35-catalog-fields.json \
  --output target/c5-touch35-catalog-entries
```

For LCD2.73, use its independently checked patch and wiring fields:

```sh
python3 tools/generate-waveshare-c5-entries.py \
  docs/research/waveshare-c5-lcd273-central-patch.json \
  --fields docs/research/waveshare-c5-lcd273-catalog-fields.json \
  --output target/c5-lcd273-catalog-entries
```

Optional `--existing-root /path/to/integration-checkout` preserves an already
reviewed product's model/dimensions and `model-*` knowledge facts/sources/assets.
An absent binding produces no claim that a model is unavailable. The model
integration lane can attach its independent result after the product is added.
`--at` accepts a UTC timestamp for reproducible regeneration; it must follow
every original source retrieval. It is the generation time, not a newly
invented download time.

The output contains `product.json`, `knowledge.json`, `firmwares.json`,
`watchlist.json`, `index-entry.json` and `integration.json`. The last file
lists the exact original diagnostic App copy operation and hash. Upsert by
exact ID, preserving independently integrated geometry. Knowledge uses string
schema version `"1"`; every fetched source has an original-byte hash, timestamp
and archive name, while microSD interfaces carry physical pins and `external`.

Original compressed source bytes are verified before generation. A zero-byte
source remains in the provenance manifest and is explicitly omitted as a
knowledge fact source. Proprietary locally retained datasheets can have a
receipt-only source row; their PDFs are not republished by this script.

The factory and any linked Xiaozhi package remain `reference` with no runnable
source. The project's independent SRAM diagnostic is `trial`; its simulation
flag is backed by the native receipt and exact App hash. Its text explicitly
limits the claim to native execution until separate public Worker/browser
evidence is integrated. No hardware success or factory success is inferred.

The generator's Touch3.5 and LCD2.73 outputs passed the local product/firmware and
watchlist JSON Schemas plus `validateKnowledge` from
`tools/platform-knowledge-test.mjs`. Integrators still run the normal whole
catalog/runtime checks after merging the fragments and registering the board.
