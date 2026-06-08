#[cfg(target_os = "windows")]
mod windows;

pub fn current_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return "windows";
    }

    #[cfg(target_os = "macos")]
    {
        return "macos";
    }

    #[cfg(target_os = "linux")]
    {
        return "linux";
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown"
    }
}

pub fn wallpaper_mode_state() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        return windows::wallpaper_mode_state();
    }

    #[cfg(not(target_os = "windows"))]
    {
        "not-supported"
    }
}
