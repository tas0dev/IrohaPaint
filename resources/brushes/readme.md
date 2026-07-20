# IrohaPaint brushes

IrohaPaint loads every `*.irohabrush` file in this directory at startup.
Use **Reload Brushes** in Brush Settings after adding or editing a file while the app is running.

```ini
version=2
name=My Brush
tip=ellipse
tip_roundness=0.75
tip_angle=-45
width=12
minimum_width=0.2
smoothing=0.7
streamline=0.5
taper_start=0.2
taper_end=0.2
color=#000000FF
cap=round
join=round
```

Values:

- `tip`: `round` or `ellipse`
- `tip_roundness`: `0.05` to `1`
- `width`: `0.1` to `256`
- `minimum_width`, `smoothing`, `streamline`, `taper_start`, `taper_end`: `0` to `1`
- `color`: `#RRGGBB` or `#RRGGBBAA`
- `cap`: `butt`, `round`, or `square`
- `join`: `miter`, `round`, or `bevel`
