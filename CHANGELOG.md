# Changelog

## Unreleased

- **Big Picture** is controller-first: left rail (Library / Files / Stats / System), wrapping 3D game wheel, LB/RB (or Q/E) to change sections.
- Groups in Big Picture use the same fanned cover stack as the desktop library. Opening a group slides covers out of the fan; closing slides them back in, then the rest of the library slides back onto the wheel.
- Grouped covers keep that group’s accent border on the desktop grid and in Big Picture, including stacked covers while the group is closed.
- System power actions (sleep, lock, restart, shut down) ask for confirmation; Cancel is the default.
- Files stays on the file list with left/right; it does not dump you onto the rail.
- Release workflow builds the tagged git commit (not the local working tree). Push related frontend and Tauri files together before tagging.
