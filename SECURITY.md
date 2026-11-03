# Security policy

## Supported versions

Only the latest commit on `main` is supported.

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Use
[GitHub private vulnerability reporting](https://github.com/cotizcesar/starcraft-re-recomp/security/advisories/new)
and include:

- the affected component (for example the `.gcsave`/`.gcrep` loader, the LAN
  protocol in `src/net.rs`, or the CASC/MPQ/CHK parsers);
- a minimal reproducer, ideally a crafted file or packet that you created
  yourself and that contains no retail game data;
- the impact you observed (crash, hang, out-of-bounds read, desync,
  arbitrary file write, …).

You will receive an acknowledgement within a week. Fixes ship with a
regression test.

## Scope notes

- Every file the engine loads is untrusted input. Loaders must cap sizes
  before allocating and reject corrupt data instead of panicking (see
  `AGENTS.md` §3).
- The LAN protocol is meant for trusted local networks. It has no
  authentication or encryption, and exposing it to the internet is
  unsupported.
- The client never writes to the StarCraft installation. A path that does is
  a security bug.
