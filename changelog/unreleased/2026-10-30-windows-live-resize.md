- **Windows draws each size a drag reaches before the window shows it.** winit doesn't report
  Win32's size-move loop, so a dragged edge asked for a redraw that landed after DWM had already
  shown the new size with the last frame stretched or clipped. A window subclass now watches the
  loop (`win32_live_resize`), and inside it `Resized` draws at once, as a macOS live resize does,
  with one frame of latency and presents that skip vsync where the surface offers them; the
  steady latency and vsync come back with the paint after the drag. On DX12 the drag keeps one
  monitor-sized drawable and draws the window's part of it (`Win::drawn_size`), so a step no
  longer rebuilds the swapchain, and DirectComposition windows drop the GDI redirection bitmap
  DWM would stretch meanwhile (`gpu_pick::windows_composition`). Measured on a 240 Hz display
  (RTX 4070), a `kit-gallery` drag step went from 5.8–8.5 ms to 3.4–6.0 ms, with the native
  share at 0.86 ms. `RUNE_WIN_SYNC_RESIZE=0`, `RUNE_WIN_RESIZE_VSYNC=1` and
  `RUNE_WIN_RESIZE_OVERSIZE=0` turn the parts off; `RUNE_RESIZE_TRACE=1` prints each drag's
  timings, and phase times with the `frame-profile` feature.
