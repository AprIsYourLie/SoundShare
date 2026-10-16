# Vendored TeamSpeak protocol crates

This directory contains the `tsclientlib 0.2.0` and `tsproto 0.2.0` sources used by SoundShare.

The baseline projects come from [ReSpeak/tsclientlib](https://github.com/ReSpeak/tsclientlib) and are licensed under MIT OR Apache-2.0. The checked-in copies include the TeamSpeak 6 compatibility changes distributed by [yichen11818/NeteaseTSBot](https://github.com/yichen11818/NeteaseTSBot). The applicable license texts are stored as `LICENSE-MIT` and `LICENSE-APACHE` in this directory.

The crates are vendored to keep the tested TS6 protocol behavior reproducible and to avoid silently changing connection behavior when an upstream branch moves.
