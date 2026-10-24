use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use libloading::Library;

use crate::error::{CoreError, Result};

type FnCreateClientId = unsafe extern "C" fn() -> c_int;
type FnSend = unsafe extern "C" fn(c_int, *const c_char);
type FnReceive = unsafe extern "C" fn(c_double) -> *const c_char;
type FnExecute = unsafe extern "C" fn(*const c_char) -> *const c_char;

/// Low-level wrapper around the C tdjson dynamic library.
pub struct TdJsonRaw {
    _lib: Arc<Library>,
    fn_create_client_id: FnCreateClientId,
    fn_send: FnSend,
    fn_receive: FnReceive,
    fn_execute: FnExecute,
}

impl TdJsonRaw {
    /// Discovers and loads the tdjson shared library from known candidate paths.
    pub fn load_auto(custom_path: Option<&Path>) -> Result<Self> {
        let mut candidates = Vec::new();

        if let Some(path) = custom_path {
            if !path.exists() {
                return Err(CoreError::LibraryNotFound(format!(
                    "Could not find tdjson dynamic library. Checked: {:?}",
                    path
                )));
            }
            return Self::load(path);
        }

        // Platform-specific library names
        let lib_names = if cfg!(target_os = "windows") {
            vec!["tdjson.dll"]
        } else if cfg!(target_os = "macos") {
            vec!["libtdjson.dylib", "libtdjson.so"]
        } else {
            vec![
                "libtdjson.so",
                "libtdjson.so.1.8.0",
                "libtdjson.so.1.8.29",
                "libtdjson.so.1.8",
                "libtdjson.so.1",
            ]
        };

        for name in &lib_names {
            // Local .data folder (portable mode)
            candidates.push(PathBuf::from(".data").join(name));
            candidates.push(PathBuf::from("..").join(".data").join(name));
            candidates.push(PathBuf::from("../..").join(".data").join(name));
            // Current directory
            candidates.push(PathBuf::from(name));
            // Next to current executable and parent directory of exe
            if let Ok(mut exe) = std::env::current_exe() {
                exe.pop();
                candidates.push(exe.join(name));
                exe.pop();
                candidates.push(exe.join(name));
                candidates.push(exe.join(".data").join(name));
            }
        }

        // On Unix/Linux, also search common system and user library paths
        #[cfg(not(target_os = "windows"))]
        {
            let sys_prefixes = [
                "/usr/lib64",
                "/usr/lib",
                "/usr/lib/x86_64-linux-gnu",
                "/usr/local/lib64",
                "/usr/local/lib",
                "/opt/tdlib/lib",
            ];
            for prefix in &sys_prefixes {
                for name in &lib_names {
                    candidates.push(PathBuf::from(prefix).join(name));
                }
            }

            // User home directories: ~/.local/share/tg-tui/, ~/.local/share/tdlib/
            if let Ok(home) = std::env::var("HOME") {
                let home_p = PathBuf::from(home);
                for name in &lib_names {
                    candidates.push(home_p.join(".local/share/tg-tui").join(name));
                    candidates.push(home_p.join(".local/share/tdlib").join(name));
                    candidates.push(home_p.join(".data").join(name));
                }
            }
        }

        if let Some(user_data) = dirs::data_dir() {
            let app_data = user_data.join("telegram-tui");
            for name in &lib_names {
                candidates.push(app_data.join(name));
            }
        }

        let mut load_errors = Vec::new();

        for path in &candidates {
            if path.exists() {
                match Self::load(path) {
                    Ok(raw) => {
                        tracing::info!("Successfully loaded tdjson from: {:?}", path);
                        return Ok(raw);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load tdjson from candidate {:?}: {}", path, e);
                        load_errors.push(format!("{}: {}", path.display(), e));
                    }
                }
            }
        }

        // Fallback: try loading from system PATH / LD_LIBRARY_PATH
        for name in &lib_names {
            match Self::load(Path::new(name)) {
                Ok(raw) => {
                    tracing::info!("Loaded tdjson from system library path: {}", name);
                    return Ok(raw);
                }
                Err(e) => {
                    tracing::debug!("System library candidate {} failed: {}", name, e);
                }
            }
        }

        let mut error_msg = format!("Could not find tdjson dynamic library. Checked: {:?}", candidates);
        if !load_errors.is_empty() {
            error_msg.push_str("\nLoad errors for candidates found on disk:\n");
            for err in &load_errors {
                error_msg.push_str(&format!("  * {}\n", err));
            }
        }

        Err(CoreError::LibraryNotFound(error_msg))
    }

    /// Loads tdjson directly from a specific file path.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        #[cfg(target_os = "windows")]
        unsafe {
            use std::os::windows::ffi::OsStrExt;
            if let Some(parent) = path.parent() {
                if let Ok(abs_parent) = std::fs::canonicalize(parent) {
                    let s = abs_parent.to_string_lossy();
                    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
                    let mut wide: Vec<u16> = std::ffi::OsStr::new(s).encode_wide().collect();
                    wide.push(0);
                    extern "system" {
                        fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
                    }
                    SetDllDirectoryW(wide.as_ptr());
                }
            }
        }

        unsafe {
            let lib = Library::new(path)
                .map_err(|e| CoreError::LibraryLoadError(format!("Failed to load {}: {}", path.display(), e)))?;
            let lib = Arc::new(lib);

            let fn_create_client_id: FnCreateClientId = *lib.get(b"td_create_client_id\0")
                .map_err(|e| CoreError::LibraryLoadError(format!("Symbol td_create_client_id missing: {}", e)))?;

            let fn_send: FnSend = *lib.get(b"td_send\0")
                .map_err(|e| CoreError::LibraryLoadError(format!("Symbol td_send missing: {}", e)))?;

            let fn_receive: FnReceive = *lib.get(b"td_receive\0")
                .map_err(|e| CoreError::LibraryLoadError(format!("Symbol td_receive missing: {}", e)))?;

            let fn_execute: FnExecute = *lib.get(b"td_execute\0")
                .map_err(|e| CoreError::LibraryLoadError(format!("Symbol td_execute missing: {}", e)))?;

            Ok(Self {
                _lib: lib,
                fn_create_client_id,
                fn_send,
                fn_receive,
                fn_execute,
            })
        }
    }

    pub fn create_client_id(&self) -> i32 {
        unsafe { (self.fn_create_client_id)() as i32 }
    }

    pub fn send(&self, client_id: i32, request: &str) -> Result<()> {
        let c_str = CString::new(request)
            .map_err(|e| CoreError::Internal(format!("Invalid CString: {}", e)))?;
        unsafe {
            (self.fn_send)(client_id as c_int, c_str.as_ptr());
        }
        Ok(())
    }

    pub fn receive(&self, timeout: f64) -> Option<String> {
        unsafe {
            let ptr = (self.fn_receive)(timeout as c_double);
            if ptr.is_null() {
                None
            } else {
                CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
            }
        }
    }

    pub fn execute(&self, request: &str) -> Option<String> {
        let c_str = CString::new(request).ok()?;
        unsafe {
            let ptr = (self.fn_execute)(c_str.as_ptr());
            if ptr.is_null() {
                None
            } else {
                CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
            }
        }
    }
}
