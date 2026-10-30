- **A window whose root has no fill clears to the page under it.** `Runtime::clear_color` took
  the app root's fill and made it opaque, so a root that fills nothing (a kit root around a
  page) cleared to black. It now takes the first of the next 64 elements under the root that
  covers the window with an opaque fill, and black only when none does. A root with a fill
  clears as before. The clear shows where a frame doesn't reach: past a Windows live resize's
  last frame, or behind a transparent corner.
