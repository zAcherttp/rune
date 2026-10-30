//! What a window holds per OS: on macOS its display link, present hook, panel and native
//! views; on Linux desktops its activation helper; on Windows its size-move loop; elsewhere
//! nothing.

/// Dropped before the surface and the window, so its AppKit observers and views go while both
/// still exist.
#[cfg(target_os = "macos")]
pub(super) struct Native {
    pub(super) pacing: Option<crate::platform::display_link::DisplayLink>,
    /// Reports when each drawable reached the screen, while tracing or for `shadow`.
    pub(super) present: Option<crate::frame::present_time::metal::LayerHook>,
    /// Re-samples a transparent window's system shadow once its new pixels land.
    pub(super) shadow: Option<crate::platform::macos::window_shape::Shadow>,
    pub(super) panel: Option<crate::platform::macos::Panel>,
    /// `El::system_material` views behind the Metal layer.
    pub(super) materials: crate::system_material::macos::Materials,
    pub(super) help: crate::platform::macos::help::NativeHelp,
    /// Redraws on full screen changes.
    pub(super) full_screen: super::mac::FullScreenWatch,
    /// Fn presses, which winit doesn't send, while the app wants modifier keys.
    pub(super) fn_key: Option<crate::input::fn_key::FnKey>,
    /// Initial frames (`Host::try_create` and one follow-up) that may draw before AppKit
    /// reports the new window on screen.
    pub(super) prime: u8,
    pub(super) launch_drawables: super::LaunchDrawables,
    /// Forgets the window's traffic-light spot.
    pub(super) _spot: crate::platform::macos::traffic_lights::Spot,
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "android")))]
#[derive(Default)]
pub(super) struct Native {
    pub(super) activator: Option<crate::platform::activation::Activator>,
}

#[cfg(target_os = "windows")]
pub(super) struct Native {
    /// The size-move loop, in which a drag draws each size in its `Resized`.
    pub(super) live: Option<crate::platform::win32::live_resize::LiveResize>,
    /// A drag may keep one drawable larger than the window (`Win::resize_live`): DX12 through
    /// DirectComposition shows it unscaled, clipped to the window. `RUNE_WIN_RESIZE_OVERSIZE=0`
    /// rebuilds the swapchain at every size instead.
    pub(super) oversize: bool,
    /// The window's size while the drawable is larger; frames draw at this size.
    pub(super) drawn: Option<(u32, u32)>,
}

#[cfg(target_os = "windows")]
impl Native {
    /// `immediate` presents are offered only where the surface lists them (`platform::win32::live_resize`).
    pub(super) fn new(window: &winit::window::Window, surface: &wgpu::Surface<'_>, adapter: Option<&wgpu::Adapter>) -> Self {
        let immediate = adapter.is_some_and(|a| surface.get_capabilities(a).present_modes.contains(&wgpu::PresentMode::Immediate));
        let oversize = adapter.is_some_and(|a| a.get_info().backend == wgpu::Backend::Dx12) && crate::gpu::pick::windows_composition()
            && std::env::var("RUNE_WIN_RESIZE_OVERSIZE").map_or(true, |v| v != "0");
        Self { live: crate::platform::win32::live_resize::LiveResize::new(window, immediate), oversize, drawn: None }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", all(unix, not(target_os = "ios"), not(target_os = "android")))))]
#[derive(Default)]
pub(super) struct Native;
