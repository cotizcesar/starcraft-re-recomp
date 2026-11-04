# Visual parity report (44 references)

> **Historical (2026-10-29), superseded.**  These numbers predate the
> retail-art rebuilds of the menus and the in-game rounds 1–3.  The live
> measurement is `engine-rust/docs/gates/ui-visual-diff.txt`
> (`scripts/ui_visual_diff.py`, 2560x1440 content box): **51/51 screens
> pass** (mean ≤ 8.0, outliers ≤ 8 %) with a mean of 2.10 and the worst at
> 6.99 (`saved`); the eight (8)The Hunters in-game references are at
> 2.44–3.27 raw (`ui-visual-diff-raw.txt`).  What is still open is listed
> in `PROGRESS.md`.

Metric: mean absolute per-channel difference (0..255) between our
headless render at 3440x1440 and the corresponding retail capture.
Lower is better; 0 would be identical (impossible with a different map state).

| # | Reference | Our screen | mean\|diff\| |
|---:|---|---|---:|
| 1 | Screenshot_20261027_130637.png | menu | 4.34 |
| 2 | Screenshot_20261027_130704.png | menu | 4.88 |
| 3 | Screenshot_20261027_130712.png | menu | 4.74 |
| 4 | Screenshot_20261027_130727.png | gametype | 5.57 |
| 5 | Screenshot_20261027_130734.png | registry | 9.37 |
| 6 | Screenshot_20261027_130744.png | episode | 7.17 |
| 7 | Screenshot_20261027_130750.png | episode | 8.23 |
| 8 | Screenshot_20261027_130757.png | episode | 8.05 |
| 9 | Screenshot_20261027_130804.png | episode | 8.74 |
| 10 | Screenshot_20261027_130824.png | episode | 8.07 |
| 11 | Screenshot_20261027_130832.png | episode | 7.90 |
| 12 | Screenshot_20261027_130840.png | episode | 8.42 |
| 13 | Screenshot_20261027_130848.png | saved | 5.23 |
| 14 | Screenshot_20261027_130858.png | replays | 10.01 |
| 15 | Screenshot_20261027_130907.png | replays | 10.72 |
| 16 | Screenshot_20261027_130926.png | menu | 4.62 |
| 17 | Screenshot_20261027_130933.png | options-speed | 10.79 |
| 18 | Screenshot_20261027_130938.png | options-sound | 10.52 |
| 19 | Screenshot_20261027_130942.png | options-video | 10.79 |
| 20 | Screenshot_20261027_130949.png | options-game | 10.79 |
| 21 | Screenshot_20261027_130954.png | options-network | 10.16 |
| 22 | Screenshot_20261027_131005.png | collections | 14.05 |
| 23 | Screenshot_20261027_131015.png | hotkeys-terran | 6.22 |
| 24 | Screenshot_20261027_131026.png | hotkeys-protoss | 8.01 |
| 25 | Screenshot_20261027_131037.png | hotkeys-zerg | 8.84 |
| 26 | Screenshot_20261027_131049.png | hotkeys-zerg | 7.74 |
| 27 | Screenshot_20261027_131101.png | credits | 2.86 |
| 28 | Screenshot_20261027_131116.png | gametype | 5.52 |
| 29 | Screenshot_20261027_131132.png | episode | 7.81 |
| 30 | Screenshot_20261027_131147.png | ingame-terran | 20.95 |
| 31 | Screenshot_20261027_131153.png | ingame-terran | 20.85 |
| 32 | Screenshot_20261027_131201.png | gamemenu | 18.32 |
| 33 | Screenshot_20261027_131209.png | ingame-terran | 20.84 |
| 34 | Screenshot_20261027_131220.png | ingame-terran | 19.85 |
| 35 | Screenshot_20261027_134831.png | replays | 11.84 |
| 36 | Screenshot_20261027_134843.png | replays | 11.96 |
| 37 | Screenshot_20261027_134905.png | ingame-zerg | 18.77 |
| 38 | Screenshot_20261027_134919.png | objectives | 15.09 |
| 39 | Screenshot_20261027_134931.png | tips | 16.14 |
| 40 | Screenshot_20261027_135020.png | ingame-terran2 | 15.75 |
| 41 | Screenshot_20261027_135051.png | ingame-protoss | 17.90 |
| 42 | Screenshot_20261027_135109.png | ingame-protoss | 17.98 |
| 43 | Screenshot_20261027_135114.png | ingame-protoss | 17.85 |
| 44 | Screenshot_20261027_135121.png | ingame-protoss | 17.94 |

Covered: **44/44**. Global mean **11.19**, best **2.86**, worst **20.95**.
