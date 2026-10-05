//! Windows GPU adapter enumeration and per-process GPU preference configuration.
//!
//! Controls which graphics adapter (Dedicated High-Performance vs. Integrated
//! Power-Saving) Windows assigns to Minecraft's Java runtime process (`javaw.exe`
//! and `java.exe`).
//!
//! Windows Display Driver Model (WDDM 2.4+ / Windows 10 build 17134+) manages
//! per-application GPU preference in the current user's registry hive:
//! `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`.
//!
//! This registry key can be read and written by any standard user-level process
//! with zero Administrator or UAC elevation required.

use std::path::Path;
use serde::{Deserialize, Serialize};

/// Graphics processor preference for running Minecraft instances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GpuPreference {
    /// Dedicated GPU (NVIDIA RTX / AMD Radeon) for maximum FPS, shaders, and VR.
    #[default]
    Dedicated,
    /// Integrated GPU (Intel Iris / AMD APU) for battery conservation.
    Integrated,
    /// Windows system default heuristic.
    System,
}

impl GpuPreference {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dedicated => "dedicated",
            Self::Integrated => "integrated",
            Self::System => "system",
        }
    }
}

/// Information about a detected GPU adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterInfo {
    pub name: String,
    pub is_dedicated: bool,
    pub vram_mb: u64,
    pub vendor_id: u32,
}

/// Complete system GPU report provided to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuSystemInfo {
    pub supported: bool,
    pub adapters: Vec<GpuAdapterInfo>,
    pub preferred: GpuPreference,
    pub active_gpu_name: Option<String>,
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct DxgiAdapterDesc {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    sub_sys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    adapter_luid: [u32; 2],
}

#[cfg(target_os = "windows")]
extern "system" {
    fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut std::ffi::c_void;
    fn GetProcAddress(h_module: *mut std::ffi::c_void, lp_proc_name: *const u8) -> *mut std::ffi::c_void;
    fn FreeLibrary(h_module: *mut std::ffi::c_void) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(
        hKey: isize,
        lpSubKey: *const u16,
        Reserved: u32,
        lpClass: *mut u16,
        dwOptions: u32,
        samDesired: u32,
        lpSecurityAttributes: *mut std::ffi::c_void,
        phkResult: *mut isize,
        lpdwDisposition: *mut u32,
    ) -> i32;

    fn RegSetValueExW(
        hKey: isize,
        lpValueName: *const u16,
        Reserved: u32,
        dwType: u32,
        lpData: *const u8,
        cbData: u32,
    ) -> i32;

    fn RegDeleteValueW(
        hKey: isize,
        lpValueName: *const u16,
    ) -> i32;

    fn RegCloseKey(
        hKey: isize,
    ) -> i32;
}

#[cfg(target_os = "windows")]
const HKEY_CURRENT_USER: isize = -2147483647; // 0x80000001 as signed isize
#[cfg(target_os = "windows")]
const KEY_SET_VALUE: u32 = 0x0002;
#[cfg(target_os = "windows")]
const KEY_QUERY_VALUE: u32 = 0x0001;
#[cfg(target_os = "windows")]
const REG_SZ: u32 = 1;
#[cfg(target_os = "windows")]
const REG_OPTION_NON_VOLATILE: u32 = 0;
#[cfg(target_os = "windows")]
const ERROR_SUCCESS: i32 = 0;

#[cfg(target_os = "windows")]
fn to_wide_null(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Enumerates graphics adapters using DXGI.
#[cfg(target_os = "windows")]
pub fn enumerate_dxgi_adapters() -> Vec<GpuAdapterInfo> {
    use std::ffi::c_void;

    let mut adapters = Vec::new();

    unsafe {
        let dxgi = LoadLibraryA(b"dxgi.dll\0".as_ptr());
        if dxgi.is_null() {
            return adapters;
        }

        let create_factory_ptr = GetProcAddress(dxgi, b"CreateDXGIFactory1\0".as_ptr());
        if create_factory_ptr.is_null() {
            FreeLibrary(dxgi);
            return adapters;
        }

        let iid_idxgi_factory1: [u8; 16] = [
            0x78, 0xae, 0x0a, 0x77, 0x6f, 0xf2, 0xba, 0x4d,
            0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87,
        ];

        type CreateFactory1Fn = unsafe extern "system" fn(*const [u8; 16], *mut *mut c_void) -> i32;
        let create_factory1: CreateFactory1Fn = std::mem::transmute(create_factory_ptr);

        let mut factory: *mut c_void = std::ptr::null_mut();
        if create_factory1(&iid_idxgi_factory1, &mut factory) != 0 || factory.is_null() {
            FreeLibrary(dxgi);
            return adapters;
        }

        let vtable = *(factory as *mut *mut *mut c_void);
        let enum_adapters1_ptr = *vtable.add(12);
        type EnumAdapters1Fn = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        let enum_adapters1: EnumAdapters1Fn = std::mem::transmute(enum_adapters1_ptr);

        let mut idx = 0u32;
        loop {
            let mut adapter: *mut c_void = std::ptr::null_mut();
            if enum_adapters1(factory, idx, &mut adapter) != 0 || adapter.is_null() {
                break;
            }

            let avtable = *(adapter as *mut *mut *mut c_void);
            let get_desc_ptr = *avtable.add(8);
            type GetDescFn = unsafe extern "system" fn(*mut c_void, *mut DxgiAdapterDesc) -> i32;
            let get_desc: GetDescFn = std::mem::transmute(get_desc_ptr);

            let mut desc = std::mem::zeroed::<DxgiAdapterDesc>();
            if get_desc(adapter, &mut desc) == 0 {
                // Microsoft Basic Render Driver / Software rasterizer vendor ID is 0x1414
                if desc.vendor_id != 0x1414 {
                    let name = String::from_utf16_lossy(&desc.description)
                        .trim_matches(char::from(0))
                        .trim()
                        .to_string();

                    let vram_mb = (desc.dedicated_video_memory / (1024 * 1024)) as u64;

                    // NVIDIA (0x10DE) or discrete AMD (0x1002 with >= 512MB VRAM) or any GPU with >= 512MB dedicated VRAM
                    let is_dedicated = desc.vendor_id == 0x10DE
                        || (desc.vendor_id == 0x1002 && vram_mb >= 512)
                        || vram_mb >= 512;

                    if !name.is_empty() {
                        adapters.push(GpuAdapterInfo {
                            name,
                            is_dedicated,
                            vram_mb,
                            vendor_id: desc.vendor_id,
                        });
                    }
                }
            }

            let release_ptr = *avtable.add(2);
            type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
            let release: ReleaseFn = std::mem::transmute(release_ptr);
            release(adapter);

            idx += 1;
        }

        let release_ptr = *vtable.add(2);
        type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
        let release: ReleaseFn = std::mem::transmute(release_ptr);
        release(factory);
        FreeLibrary(dxgi);
    }

    adapters
}

/// Applies the requested GPU preference to the Windows UserGpuPreferences registry key
/// for the specified Java executable.
#[cfg(target_os = "windows")]
pub fn apply_gpu_preference(executable_path: &Path, preference: GpuPreference) -> Result<(), String> {
    let path_str = executable_path.to_string_lossy();
    // Normalize path by stripping \\?\ prefix if present
    let clean_path = if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path_str.to_string()
    };

    let subkey = to_wide_null("Software\\Microsoft\\DirectX\\UserGpuPreferences");
    let mut hkey: isize = 0;
    let mut disposition: u32 = 0;

    unsafe {
        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            std::ptr::null_mut(),
            &mut hkey,
            &mut disposition,
        );

        if res != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW failed with code {res}"));
        }

        let val_name = to_wide_null(&clean_path);

        let set_res = match preference {
            GpuPreference::Dedicated => {
                let data = to_wide_null("GpuPreference=2;");
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr() as *const u8,
                    (data.len() * 2) as u32,
                )
            }
            GpuPreference::Integrated => {
                let data = to_wide_null("GpuPreference=1;");
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr() as *const u8,
                    (data.len() * 2) as u32,
                )
            }
            GpuPreference::System => {
                // Delete explicit preference so Windows default takes effect
                let del_res = RegDeleteValueW(hkey, val_name.as_ptr());
                if del_res == ERROR_SUCCESS || del_res == 2 /* ERROR_FILE_NOT_FOUND */ {
                    ERROR_SUCCESS
                } else {
                    let data = to_wide_null("GpuPreference=0;");
                    RegSetValueExW(
                        hkey,
                        val_name.as_ptr(),
                        0,
                        REG_SZ,
                        data.as_ptr() as *const u8,
                        (data.len() * 2) as u32,
                    )
                }
            }
        };

        RegCloseKey(hkey);

        if set_res != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW failed with code {set_res}"));
        }
    }

    tracing::info!(
        "Configured Windows GPU preference '{}' for executable: {}",
        preference.as_str(),
        clean_path
    );

    // Companion binary registration: if javaw.exe was configured, also configure java.exe (and vice-versa)
    let path = Path::new(&clean_path);
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        let companion_name = if file_name.eq_ignore_ascii_case("javaw.exe") {
            Some("java.exe")
        } else if file_name.eq_ignore_ascii_case("java.exe") {
            Some("javaw.exe")
        } else {
            None
        };

        if let Some(comp) = companion_name {
            let companion_path = path.with_file_name(comp);
            if companion_path.exists() && companion_path != path {
                let _ = apply_gpu_preference_single(&companion_path, preference);
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_gpu_preference_single(executable_path: &Path, preference: GpuPreference) -> Result<(), String> {
    let path_str = executable_path.to_string_lossy();
    let clean_path = if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        path_str.to_string()
    };

    let subkey = to_wide_null("Software\\Microsoft\\DirectX\\UserGpuPreferences");
    let mut hkey: isize = 0;
    let mut disposition: u32 = 0;

    unsafe {
        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            std::ptr::null_mut(),
            &mut hkey,
            &mut disposition,
        );

        if res != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW failed with code {res}"));
        }

        let val_name = to_wide_null(&clean_path);

        let set_res = match preference {
            GpuPreference::Dedicated => {
                let data = to_wide_null("GpuPreference=2;");
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr() as *const u8,
                    (data.len() * 2) as u32,
                )
            }
            GpuPreference::Integrated => {
                let data = to_wide_null("GpuPreference=1;");
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr() as *const u8,
                    (data.len() * 2) as u32,
                )
            }
            GpuPreference::System => {
                let del_res = RegDeleteValueW(hkey, val_name.as_ptr());
                if del_res == ERROR_SUCCESS || del_res == 2 {
                    ERROR_SUCCESS
                } else {
                    let data = to_wide_null("GpuPreference=0;");
                    RegSetValueExW(
                        hkey,
                        val_name.as_ptr(),
                        0,
                        REG_SZ,
                        data.as_ptr() as *const u8,
                        (data.len() * 2) as u32,
                    )
                }
            }
        };

        RegCloseKey(hkey);

        if set_res != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW failed with code {set_res}"));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn apply_gpu_preference(_executable_path: &Path, _preference: GpuPreference) -> Result<(), String> {
    Ok(())
}

/// Returns the detected GPU adapters and current preference.
pub fn get_gpu_system_info(preferred: GpuPreference) -> GpuSystemInfo {
    #[cfg(target_os = "windows")]
    {
        let adapters = enumerate_dxgi_adapters();
        let active_gpu_name = match preferred {
            GpuPreference::Dedicated => adapters.iter().find(|a| a.is_dedicated).map(|a| a.name.clone()),
            GpuPreference::Integrated => adapters.iter().find(|a| !a.is_dedicated).map(|a| a.name.clone()),
            GpuPreference::System => None,
        }
        .or_else(|| adapters.first().map(|a| a.name.clone()));

        GpuSystemInfo {
            supported: !adapters.is_empty(),
            adapters,
            preferred,
            active_gpu_name,
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        GpuSystemInfo {
            supported: false,
            adapters: Vec::new(),
            preferred,
            active_gpu_name: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preference_strings() {
        assert_eq!(GpuPreference::Dedicated.as_str(), "dedicated");
        assert_eq!(GpuPreference::Integrated.as_str(), "integrated");
        assert_eq!(GpuPreference::System.as_str(), "system");
    }

    #[test]
    fn test_system_info() {
        let info = get_gpu_system_info(GpuPreference::Dedicated);
        assert_eq!(info.preferred, GpuPreference::Dedicated);
    }
}
