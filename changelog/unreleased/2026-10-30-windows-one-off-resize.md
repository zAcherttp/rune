- **A Windows resize outside a drag draws before `Resized` returns.** Maximise, restore, Snap and
  a scale change arrive as one `WM_SIZE` outside the size-move loop, and asked for a redraw that
  landed after it returned. They now draw in `Resized`, at the steady latency and with vsync, so
  the frame for the new size is presented before DWM shows it. Windows already hides most of
  the old frame behind its maximise and Snap animations, so on one machine (RTX 4070, 240 Hz)
  the change was hard to see, and `Resized`-to-drawn time was the same (4–6 ms). Sizes Rune
  steps itself and windows before their first frame keep the deferred redraw.
  `RUNE_WIN_SYNC_RESIZE=0` turns it off with the drag path.
