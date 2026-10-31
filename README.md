# NIRUAMP

**A Winamp-inspired Rust frontend for [NPLAY](https://github.com/nirucon/nplay), designed for the [NIRUX](https://github.com/nirucon/nirux) desktop.**

Compact, fast, resource-conscious and unmistakably inspired by classic Winamp: a distinctive player deck, digital time display, visualization and playlist, styled beautifully with NIRUX themes.

## Architecture

NIRUAMP is a graphical client, not a second playback engine. **NPLAY is a required runtime dependency** and remains fully usable as a standalone terminal player. Playback, providers, queue, library and listening statistics belong to NPLAY; NIRUAMP owns presentation and graphical interaction.

Implementation will add a versioned local control interface to NPLAY while preserving its existing terminal experience. The frontend itself is written in Rust; the NPLAY backend remains Python.

## Status

**Planning / implementation handoff. No runnable player yet.**

See [AGENTS.md](AGENTS.md) for the implementation brief, architecture constraints, autonomous workflow and acceptance criteria. The owner will provide authorized bare-metal build/test machines and SSH details to the implementing agent.

## Design priorities

- Immediately recognizable classic Winamp inspiration, with original artwork and identity.
- Excellent keyboard and mouse UX, crisp scaling and accessible controls.
- Live NIRUX theme integration, plus a standalone fallback appearance.
- Low idle CPU use, bounded caches and no unnecessary background work.
- One authoritative NPLAY engine, without duplicated providers or statistics.

MIT licensed. Independent project; not affiliated with Winamp.
