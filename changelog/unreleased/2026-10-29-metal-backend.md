- **A native Metal backend, opt-in.** With the `metal-backend` feature on macOS, launching with
  `RUNE_GPU_BACKEND=metal` draws through Rune's own Metal backend, plugged in under wgpu's API
  through wgpu's `custom` backend, instead of wgpu's; wgpu stays the default and the feature
  changes nothing without the variable. It draws the same pixels as wgpu. `NativeSurface` video
  needs wgpu's backend and says so once on stderr; compute, queries, render bundles, depth and
  multisampling aren't implemented there, so a compositor that needs them stays on wgpu.
