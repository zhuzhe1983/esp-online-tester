# Near-ground ST-driver check

The unchanged M5StampFly ST VL53L3CX driver processes the existing Rust histogram model at bottom address0x29, MEDIUM,33ms. Ideal35/50/75/100mm inputs settle to42/57/79/99mm with status0; startup invalid frames and increasing-distance history remain observable. This native protocol check is separate from ESP32 guest execution and physical calibration.

Run `python3 verify-nearground.py <pinned-M5StampFly-source> --root <repository> --output <receipt.txt>` from this directory. The script verifies the pinned76-file source aggregate and sensor module hash, and builds in a temporary directory. See `report.json`, `samples.json` and `provenance.json` for all312 frames, negative statuses, commands and historical hashes. Repeated stdout was byte-identical. Source paths in commands are normalized; no measured values are changed.

The optional flight scene uses a conservative50mm visibility cutoff. This is a scene assumption, not the ST algorithm's minimum range. No-target is never replaced by a fake valid minimum or calibrated zero. The firmware must acquire valid feedback after lift and handle its loss when landing.
