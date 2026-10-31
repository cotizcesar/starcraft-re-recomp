# CEF host probe (Options panel)

The Remastered options/hotkeys/collections panels are React bundles hosted by
CEF.  They open a WebSocket to the game host and wait for `GetStrings` /
`ShowTab`.  `scripts/cef_host_probe.py` implements that host in Python and
renders the real panel in Chromium.

## Run

```bash
# extracted webui bundle and enus string table (local /tmp workspace)
python3 scripts/cef_host_probe.py
```

The probe:
- serves `optionspanel.html/js` over HTTP;
- accepts `ws://127.0.0.1:<port>/panel/OptionsPanel/<guid>`;
- answers `GetStrings` with the 139 ids requested by the panel, using
  `locales/enus/assets/rez/gluall.json`;
- sends `ShowTab` with the Speed settings.

## Evidence

`docs/cef-options-host.png` is the real CEF panel at a 1920x1200 viewport:

```text
CEF host render: 1920x1200
CEF panel red bbox: (196,144)..(1723,1079)
CEF panel size: 1528x936
reference panel: (519,180)..(2040,1142), size 1522x963
```

The width matches the retail panel within 6 px (0.4%) and the height within
27 px (2.8%).  This confirms that the remaining chrome is genuinely CEF, and
that a CEF-generated panel can replace the native approximation.

## Not wired yet

The probe is standalone.  Wiring it into `sc_view` requires one of:

1. Adding Playwright/Chromium as a build/runtime dependency and generating
   per-tab PNGs into the cache; or
2. Shipping a native CEF host (the install only has Windows `libcef.dll`); or
3. Pre-rendering the panels at install time with a user-provided Chromium.

The panel still contains the game's hotkey markup (control characters), so
the string decoder and the tab-label spans need to be finalized before the
PNG is used as a production texture.

The 3D chrome for Episode/Create/Mission/Replays is not CEF and is still
missing; `glue/campaign/ptopl.dds`/`ptopr.dds` are black masks (alpha 0).

## Why the probe is not a drop-in replacement yet

The rendered CEF panel was cropped to the retail panel rect and compared with
the 130933 reference:

```text
native Options panel:  mean 7.26, >30 px 5.85%
CEF host panel:        mean 40.23, >30 px 24.08%
```

Side-by-side: `docs/cef-vs-native-options.png`.

The CEF page as served by the probe has an opaque black body, a different
viewport/zoom and the wrong translucent theme; the game host supplies that
configuration.  Wiring this exact probe into `sc_view` would regress Options,
so the native reconstruction remains the better renderer until the real host
viewport/CSS/background values are captured.

The missing data is only obtainable from the running game's CEF host (remote
debugging / DevTools or a packet capture).  That capture is the next manual
step; without it, the CEF path cannot beat the native panel.
