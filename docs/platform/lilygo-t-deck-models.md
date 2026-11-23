# LILYGO T-Deck and T-Deck Plus 3D models

T-Deck and T-Deck Meshtastic use the same manufacturer board assembly. The retained STEP files and the exported triangle arrays match across the two editions. Both use the original photo-aligned keyboard and trackball. Inspection lifts the LCD and its supports 18 mm along the front axis and keeps the keyboard, trackball and soldered connectors on the board. [LILYGO T-Deck CAD](https://github.com/Xinyuan-LilyGO/T-Deck/tree/2027dee79198af949a9e14e1a09e301f993fc7cf/shell)

T-Deck Plus and T-Deck Plus Meshtastic use the same manufacturer enclosure STEP. An official front photo locates the 58.77 × 44.08 mm modeled LCD opening, the 35 keycaps and the trackball. The trackball uses the matched original T-Deck CAD component. The recorded photo calibration in each `model-plan.json` pins the source, crop, scaling, display mask and control locations. Keycap and fascia thicknesses are viewing estimates. The preview depicts the photographed configuration with its external antenna removed. Plus PCB and battery geometry remain unverified. [LILYGO T-Deck Plus documentation](https://wiki.lilygo.cc/products/t-deck-series/t-deck-plus/) · [Front reference photo](https://cdn.shopify.com/s/files/1/0617/7190/7253/files/T-DECK-PLUS_6.jpg?v=1748422600)

Plus inspection lifts the front assembly 22 mm. The screen, keys, trackball and their hit regions travel with the front assembly. The native rear shell and side inserts stay seated. Browser checks cover front and oblique hit testing, rear occlusion, the active display window, and exact restoration after moving the inspection slider through 0%, 50%, 100% and back to 0%.

Rebuild into a separate output directory with the dependencies from `tools/requirements-model.txt` installed.

```sh
python tools/build-lilygo-models.py --output /tmp/t-deck-models lilygo-t-deck
python tools/build-lilygo-expansion-models.py --output /tmp/t-deck-models \
  lilygo-t-deck-meshtastic lilygo-t-deck-plus-1 lilygo-t-deck-plus-meshtastic
```
