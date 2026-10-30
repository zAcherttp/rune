//! The Win32 size-move loop, which winit 0.30 doesn't report. DWM shows each size a drag
//! reaches when `WM_SIZE` returns, so inside the loop a window draws in `Resized`, at one frame
//! of latency, as a macOS live resize does. A size that comes outside the loop (maximise,
//! restore, Snap, a scale change) draws in its `Resized` too, at the steady latency.
//! `RUNE_WIN_SYNC_RESIZE=0` keeps the deferred redraw for both; `RUNE_RESIZE_TRACE=1` prints
//! each drag's `Resized`-to-drawn times, and each one-off resize's.

use std::{cell::{Cell, RefCell}, rc::Rc, time::Instant};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

const SUBCLASS_ID: usize = 0x5255_4e53;
/// Inside the size-move loop: the frame drawn for a size is the next one shown.
pub(crate) const LIVE_LATENCY: u32 = 1;

struct State {
    moving: Cell<bool>,
    ended: Cell<bool>,
    /// The first `Resized` no frame has served yet.
    pending: Cell<Option<Instant>>,
    /// The same for a resize outside the loop, while tracing.
    once: Cell<Option<Instant>>,
    samples: RefCell<Vec<f32>>,
    /// Time in `surface.configure`, of `samples`.
    configures: RefCell<Vec<f32>>,
    resizes: Cell<u32>,
    /// Each frame's wall-clock phases (`frame::profile`), with the `frame-profile` feature.
    #[cfg(feature = "frame-profile")]
    phases: RefCell<Vec<[f64; crate::frame::profile::PHASES]>>,
    /// Each frame's time outside the framework's phases: the swapchain, the drawable, encoding
    /// and the present (`Resized`-to-drawn less commit and paint).
    #[cfg(feature = "frame-profile")]
    native: RefCell<Vec<f32>>,
}

pub(crate) struct LiveResize {
    hwnd: HWND,
    state: Rc<State>,
    sync: bool,
    /// Presents skip vsync inside the loop. wgpu's DX12 `configure` waits for the present
    /// queue to drain, which with vsync costs every queued frame's refresh on each step; DWM
    /// still composites the window, so nothing tears. `RUNE_WIN_RESIZE_VSYNC=1` keeps vsync.
    immediate: bool,
    trace: bool,
}

impl LiveResize {
    pub(crate) fn new(window: &winit::window::Window, immediate: bool) -> Option<Self> {
        let RawWindowHandle::Win32(h) = window.window_handle().ok()?.as_raw() else { return None };
        let hwnd = h.hwnd.get() as HWND;
        let state = Rc::new(State { moving: Cell::new(false), ended: Cell::new(false), pending: Cell::new(None), once: Cell::new(None), samples: RefCell::default(), configures: RefCell::default(), resizes: Cell::new(0), #[cfg(feature = "frame-profile")] phases: RefCell::default(), #[cfg(feature = "frame-profile")] native: RefCell::default() });
        if unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS_ID, Rc::as_ptr(&state) as usize) } == 0 { return None; }
        let sync = std::env::var("RUNE_WIN_SYNC_RESIZE").map_or(true, |v| v != "0");
        let trace = std::env::var("RUNE_RESIZE_TRACE").is_ok_and(|v| v != "0");
        let immediate = immediate && sync && std::env::var("RUNE_WIN_RESIZE_VSYNC").map_or(true, |v| v == "0");
        Some(Self { hwnd, state, sync, immediate, trace })
    }

    /// The present mode the surface wants now.
    pub(crate) fn present_mode(&self, steady: wgpu::PresentMode) -> wgpu::PresentMode {
        if self.immediate && self.drawing_live() { wgpu::PresentMode::Immediate } else { steady }
    }

    /// Inside the size-move loop, with the synchronous path on.
    pub(crate) fn drawing_live(&self) -> bool { self.sync && self.state.moving.get() }

    /// Whether a resize outside the loop draws in its `Resized`.
    pub(crate) fn draws_resizes(&self) -> bool { self.sync }

    /// A resize outside the loop, timed to its frame while tracing.
    pub(crate) fn resized_once(&self) {
        if self.trace && !self.state.moving.get() && self.state.once.get().is_none() { self.state.once.set(Some(Instant::now())); }
    }

    /// The frame latency the surface wants now.
    pub(crate) fn latency(&self, steady: u32) -> u32 { if self.drawing_live() { LIVE_LATENCY } else { steady } }

    pub(crate) fn resized(&self) {
        if !self.trace || !self.state.moving.get() { return; }
        self.state.resizes.set(self.state.resizes.get() + 1);
        if self.state.pending.get().is_none() {
            self.state.pending.set(Some(Instant::now()));
            // Phases from here on are this frame's.
            #[cfg(feature = "frame-profile")]
            crate::frame::profile::take();
        }
    }

    pub(crate) fn configured(&self, since: Instant) {
        if self.trace && self.state.moving.get() { self.state.configures.borrow_mut().push(since.elapsed().as_secs_f32() * 1000.0); }
    }

    /// A frame finished drawing.
    pub(crate) fn drew(&self) {
        if let Some(t) = self.state.once.take() {
            eprintln!("rune: resize outside a drag ({}): Resized→drawn {:.2} ms", if self.sync { "sync" } else { "deferred" }, t.elapsed().as_secs_f32() * 1000.0);
        }
        let Some(t) = self.state.pending.take() else { return };
        let ms = t.elapsed().as_secs_f32() * 1000.0;
        self.state.samples.borrow_mut().push(ms);
        #[cfg(feature = "frame-profile")]
        {
            let times = crate::frame::profile::take();
            // Commit holds view, layout, text and apply; paint runs beside it.
            self.state.native.borrow_mut().push(ms - (times[1] + times[4]) as f32);
            self.state.phases.borrow_mut().push(std::array::from_fn(|i| times[i]));
        }
    }

    /// True once after the loop ends; prints the drag's numbers when tracing.
    pub(crate) fn take_ended(&self) -> bool {
        if !self.state.ended.replace(false) { return false; }
        self.state.pending.set(None);
        let mut ms = std::mem::take(&mut *self.state.samples.borrow_mut());
        let mut cfg = std::mem::take(&mut *self.state.configures.borrow_mut());
        let resizes = self.state.resizes.replace(0);
        if self.trace && !ms.is_empty() {
            let at = |v: &mut Vec<f32>, q: f32| {
                if v.is_empty() { return 0.0; }
                v.sort_by(f32::total_cmp);
                v[((v.len() - 1) as f32 * q).round() as usize]
            };
            eprintln!(
                "rune: live resize ({}): {resizes} resizes, {} frames; Resized→drawn ms median {:.2}, p95 {:.2}, max {:.2}; configure ms median {:.2}, p95 {:.2}",
                match (self.sync, self.immediate) { (false, _) => "deferred", (true, false) => "sync, vsync", (true, true) => "sync, immediate" }, ms.len(), at(&mut ms, 0.5), at(&mut ms, 0.95), at(&mut ms, 1.0), at(&mut cfg, 0.5), at(&mut cfg, 0.95),
            );
            // Medians; Commit and Layout include their nested phases (`frame::profile`).
            #[cfg(feature = "frame-profile")]
            {
                let rows = std::mem::take(&mut *self.state.phases.borrow_mut());
                let names = ["view", "commit", "layout", "text", "paint", "apply"];
                let line: Vec<String> = names.iter().enumerate().map(|(i, name)| {
                    let mut v: Vec<f32> = rows.iter().map(|r| r[i] as f32).collect();
                    format!("{name} {:.2}", at(&mut v, 0.5))
                }).collect();
                let mut native = std::mem::take(&mut *self.state.native.borrow_mut());
                eprintln!("rune: live resize phases ms median: {}; native median {:.2}, p95 {:.2}", line.join(", "), at(&mut native, 0.5), at(&mut native, 0.95));
            }
        }
        true
    }
}

impl Drop for LiveResize {
    fn drop(&mut self) {
        unsafe { RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS_ID); }
    }
}

unsafe extern "system" fn subclass(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _: usize, data: usize) -> LRESULT {
    // SAFETY: `data` is the `State` the owning `LiveResize` keeps alive until it removes this subclass.
    let state = unsafe { &*(data as *const State) };
    match message {
        WM_ENTERSIZEMOVE => state.moving.set(true),
        WM_EXITSIZEMOVE => {
            state.moving.set(false);
            state.ended.set(true);
            // The paint restores the steady latency and steps the motion the drag held.
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0); }
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
