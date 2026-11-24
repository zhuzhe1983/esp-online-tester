# Advertised Browser/CUA fallback assessment

**Result: blocked before model execution; 0 of 16 browser profiles executed.**

This is separate from the existing `../browser-final16/` headless Chromium
attempt. That earlier attempt failed during process launch with an operating
system socket restriction and remains unchanged.

A normal Python HTTP preview successfully listened on the task's loopback
address. The advertised `mcp__cua_repl` Browser API selected its supported cloud
Chrome, and a newly created task-specific tab attempted the exact project
preview URL. Navigation returned `net::ERR_CONNECTION_REFUSED`.

The documented browser troubleshooting guidance was then read. Attempting to
bind only that newly created failure tab, in order to inspect the failure and
read its issue-reporting capability, was rejected by the browser URL policy.
The tool stated that it could not grant authority to observe the page content
and explicitly prohibited workarounds, indirect execution, raw CDP, alternate
browser surfaces and policy circumvention. No further browser action was tried.

No existing user tabs were enumerated, no account or login was probed, and no
project files were externally published. No sandbox, network path, permission
or browser security configuration was changed. The task-local preview server
was stopped through its own managed execution session, which exited with code
130; no other process was stopped.

`assessment.json` records the observed steps, the existing attempt's receipt
hash, the selection hash and all 16 current model/plan hashes. No model canvas,
DOM interaction, screenshot, iframe restoration or browser RPC test ran in
this fallback. The connection failure does not prove the cloud browser's
network topology, and the denied page's internal URL was not inspected.

These are environment/tool blockers. They are neither model failures nor
passed browser gates. Source verification, production CPU loader checks,
geometric checks and visual mesh reviews remain separately reported.
