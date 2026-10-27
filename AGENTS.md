# Product platform in this fork

- Mission: an agent-configurable, human-playable and website-embeddable platform for product-level ESP32 devices.
- Product adaptation uses at most three active agent workstreams: the primary agent plus two subagents. When the primary handles another user task, keep one controller lane and one accessory lane. Otherwise the primary also adapts controllers, for two controller lanes and one accessory lane. Save unfinished work before switching assignments; continue each queue one product at a time.
- Platform UI and API: `web/platform/`; current scope and setup: `docs/platform/README.md`.
- To configure or operate the system, read `skills/esp-online-tester/SKILL.md`. It documents the implemented API; `docs/platform/research/` contains historical drafts.
- To add or refresh products, modules, 3D assets or remote firmware, read `skills/esp-product-onboarding/SKILL.md`. Prefer manufacturer CAD, retain source timestamps and verify dimensions plus photo overlays.
- Humans, embedded parents and agents use the same runtime. Verify firmware output after inputs; a 3D preview or queued input is not evidence of execution.
- Keep product-specific support truthful. PaperMono/Lite and StopWatch are partial; other catalog entries need individual runtime verification; GPS injection bypasses physical routing and timing.
- Public catalogs accept PRs; private DIY definitions stay local unless exported or shared. See `docs/platform/contributing.md`.
- Preserve buildless static deployment under a GitHub Pages repository subpath. Runtime dependencies and assets must be local or explicitly declared user firmware sources.
- Platform checks: `node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs` and a real browser check for UI changes. Build WASM and fetch ROMs first as documented.

# Checks before a pull request

- Run what CI runs, with the newest stable Rust (`rustup update stable`): CI lints and tests with it, so an older local toolchain can pass where CI fails. [CONTRIBUTING.md](CONTRIBUTING.md) lists the commands; [.github/workflows/ci.yml](.github/workflows/ci.yml) is the full set.
- Both Clippy steps must pass with warnings denied: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings`. Clippy runs before the tests in CI, so a lint failure hides every test result.
- Run the tests as CI does, ignored tests included: `ESP32SIM_ROM_DIR=<roms> cargo test --release --workspace -- --include-ignored --skip external_`.
- A test that needs inputs only a developer machine has (a local firmware build, an objdump listing, hardware) gets both `#[ignore = "<the input it needs>"]` and an `external_*` name, and fails with a message naming what it needs. The ignore keeps a plain `cargo test` green; the name keeps it out of CI, which runs ignored tests and skips only `external_*`. Keep `external` out of every other test's name: `--skip` matches substrings, so CI would skip that test silently.
- Golden outputs are bit-identical. Regenerate them (`UPDATE_GOLDENS=1`) only for an intentional change, and say in the pull request which goldens changed and why.
- In the pull request, list the checks you ran and their results.
- Register, bit, interrupt-source and clock values cite the Espressif header and its ESP-IDF version, checked against the version the firmware uses (Arduino-ESP32 2.x = IDF 4.4, 3.x = IDF 5.x); say where versions differ. Copied code keeps no "verified on silicon" notes from another chip.
- Code not in use costs nothing: with a feature flag off, instruction counts and console output are unchanged. After touching the bus, `device_set!`, interrupt routing or per-tick work, compare median CPU time against main on the hello demos (and pocket-tank for the S3 core) with identical instruction counts; CONTRIBUTING.md has the command.
- Code style: see "Code style" in CONTRIBUTING.md. Never run rustfmt on whole files (the tree isn't rustfmt-formatted); match the dense style of the file you edit, and never let guest-controlled values panic the emulator.
- Before adding a shared piece (a register block such as IO_MUX, a `device_set!` entry, a `Board` method), check open pull requests that touch the same files; build on the one that adds it rather than adding a copy.

# Maintaining pull requests

- Small, clearly correct review fixes may go straight onto the contributor's branch (maintainer edits) instead of a review round trip. Add commits; never force-push a contributor's branch.
- After 14 days with no reply or push following a review, a maintainer may finish the pull request: small fixes on the contributor's branch; larger rework merged and fixed in a follow-up, or redone in a new pull request that keeps their commits or adds a `Co-authored-by:` trailer. Leave a note on the pull request saying what changed and why.
- After 90 days without activity, close the pull request with a note; it can be reopened. A contributor asking for more time stops the clock.
- Changes to a contributor's pull request go through the same checks and claim verification as a review.

# Experiment history

- Before proposing, implementing or benchmarking an ESP32-S3 execution, browser-speed or timing experiment, search [docs/experiments.md](docs/experiments.md) by mechanism and aliases.
- Cite the existing experiment ID and say what materially differs before retrying: mechanism, workload, correctness contract or measurement quality. A renamed branch is not a new experiment. Inspect preserved patches before rebuilding them.
- Record the outcome in the same entry, keeping earlier results, with revisions, inputs, exact work and output checks, conditions, uncertainty, adoption and a receipt. Negative results count. Use a new stable ID only for a materially different idea and cross-reference related IDs. Do not start a second list.

# Evidence retention and privacy

- Keep curated evidence needed to reproduce or challenge a conclusion: source revisions, commands, inputs, artifact hashes, numeric samples, correctness checks, relevant machine/software configuration and limitations. Retain negative results.
- Minimize personal information before committing. Omit application/process inventories, command lines unrelated to the experiment, login names, hostnames, email addresses, device identifiers and session details. Keep aggregate load or anonymous CPU samples when needed to explain measurement noise. `/Users/alice` and `/home/alice` are normalized labels, not host identities.
- Review text, JSON, scripts and compressed profiles. Require caller-supplied remote targets in reusable harnesses. Do not retain raw private captures just because they were collected; do not add private originals or backups to Git.
- Evidence is reproducible facts: commands, inputs, hashes, numbers, outcomes. No agent-session narrative; cite commits that exist in this repository or name the public repository. A new `docs/experiments.md` row goes directly below the previous one (a blank line ends the table).
- Document redactions and their effect on evidence. Preserve measured values and historical artifact hashes; when sanitizing an existing receipt, record its original and sanitized file hashes so provenance remains explicit. Follow [the evidence guide](docs/evidence/README.md) and run `node tools/check-evidence-privacy.mjs` before publishing. The pattern check supplements manual review.

# GitHub pull request stacks

- Use `gh stack` when one pull request depends on another. Setting a PR's base branch alone does not register a native GitHub stack. If GitHub reports stacked pull requests as disabled for the repository, stack by ancestry and name the base pull request at the top of each description.
- For existing PRs, use `gh stack link --remote origin --base main <bottom-PR-URL> <top-PR-URL>` in dependency order, bottom to top. Follow any explicitly requested remote or base instead.
- For new stacks, use `gh stack init` and `gh stack submit`; inspect their current `--help` before acting.
- After linking existing PRs remotely, use `gh stack checkout <stack-number>` to import local tracking, then `gh stack view --json` to verify it. An untracked-branch error from `view` alone does not prove that the remote stack is absent.
- Verify the native GitHub stack registration before describing PRs as stacked. Distinguish branch ancestry from GitHub's stack feature.
- Do not rewrite published branch history merely to register an existing stack.
- If merge commits prevent `gh stack modify`, preserve published history and use `gh stack link` to add new PRs. When local tracking needs a clean refresh, use `gh stack unstack --local` followed by `gh stack checkout <stack-number>`; the `--local` flag leaves the GitHub stack intact.
