# LILYGO T-Deck Pro and T-LoRa Pager models

The plain and Meshtastic entries for each device share the same manufacturer enclosure file hashes. Each entry retains its own gallery sources and catalog firmware configuration.

## T-Deck Pro

The V1.0 HD-V1-250326 front, inner bezel, rear cover and side key retain their supplied assembled coordinates. The four STL parts measure 66.529 × 120.150 × 13.000 mm before the face details are added. LILYGO gives a 66 × 120 × 13.5 mm nominal envelope. The 59.9 × 76.9 mm front cutout seats the display cover. The 47.04 × 62.72 mm active area follows the GDEQ031T10 specification. Keycap centers come from the 35 cutouts in the front CAD, with visible legends checked against the manufacturer gallery.

The USB-C shell fits the 9.58 × 3.67 mm side opening. It stays with the estimated board as the front, bezel and rear move to separate layers. At 100%, adjacent layers have at least 15.36 mm clearance in model Z. The board outline and connector depth are estimates. Exact PCB population, battery placement and side-key electrical assignment require additional hardware evidence.

Sources include [LILYGO T-Deck Pro hardware](https://wiki.lilygo.cc/products/t-deck-series/t-deck-pro/), [the V1.0 manufacturer enclosure files](https://github.com/Xinyuan-LilyGO/T-Deck-Pro/tree/53bcfa593b1df064894a8efc5a7e15e4524b09d2/case), [the Meshtastic gallery](https://lilygo.cc/products/t-deck-pro-meshtastic), and [Good Display's panel specification](https://www.good-display.com/companyfile/831.html).

## T-LoRa Pager

The front STL is rotated to show its exterior face, with the speaker at left and rotary controls at right. The independently oriented rear shell is seated against the perimeter of the front. CAD pairs 04/05 and 06/07 deploy around opposite rear hinge seats. Each pair uses its native internal placement and a common rigid transform. Width, deployed height and depth are about 89.4 × 106.1 × 23.2 mm, compared with the nominal 89 × 106 × 23 mm envelope.

The 222 × 480 native framebuffer rotates into the 55.5 × 25.8 mm landscape CAD opening. Front keycaps and rotary controls follow the cutouts and the original gallery. The USB-C, TF slot and bottom controls use the measured shell apertures and the original three-view diagram. In the exploded view the front and rear clearances to the stationary board and its sockets are about 26.62 mm and 27.41 mm. The blank board and socket seat are estimates. The visible keyboard faces provide a physical reference and retain the current catalog input capabilities.

Sources include [LILYGO Pager hardware](https://wiki.lilygo.cc/products/t-lora-series/t-lora-pager/), [the manufacturer mechanical files](https://github.com/Xinyuan-LilyGO/LilyGoLib/tree/9116bf41ca551d75a42d16cab391fe254f21ab2c/case/t-lora-pager), [the official port diagram](https://wiki.lilygo.cc/products/t-lora-series/t-lora-pager/index/image/t-lorapager-three-views-en.jpg), and [the Meshtastic gallery](https://lilygo.cc/products/t-lora-pager-meshtastic).

Build the four plans with `python tools/build-lilygo-expansion-models.py --output <scratch> lilygo-t-deck-pro lilygo-t-deck-pro-meshtastic lilygo-t-lora-pager lilygo-t-lora-pager-meshtastic`. The browser checks compare assembled pixels before and after the explosion reset and measure the relative PCB and USB-C positions at 0%, 50% and 100%.
