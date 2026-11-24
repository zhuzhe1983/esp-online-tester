# Browser launch attempt

The existing `tools/check-waveshare-platform-browser.mjs` harness was invoked for the final 16 product identities using a fresh Chromium context and loopback HTTP server. Chromium failed during startup: `ProcessSingleton socket() failed: Operation not permitted`. The command returned exit code 1 before any product page was loaded.

**Browser/WebGL checks completed: 0 of 16.** The headless launch attempt is retained in `command.json` and the normalized `browser-command.log`. The final delivery does not claim browser acceptance from this attempt. Model source, dimension, photo registration, Three.js loader and CPU ray checks have separate current-hash receipts under `../../waveshare-batch030-models-20261124/`.

The complete reproduction command is implemented by `../run-browser-qa.py`. Run it in an environment that permits Chromium local IPC. There was no privilege escalation or change to the execution environment. No firmware was loaded and no physical hardware was accessed.
