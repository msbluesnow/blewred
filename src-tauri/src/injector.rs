//! Dynamic OBS Studio DLL Injector
//! Safely injects and initializes `obs-blewred.dll` into running `obs64.exe`
//! processes without requiring an OBS Studio restart.

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

#[cfg(windows)]
#[allow(non_snake_case, dead_code, clashing_extern_declarations)]
mod win32 {
    use super::*;

    pub type HANDLE = isize;
    pub type HMODULE = isize;
    pub type BOOL = i32;

    pub const INVALID_HANDLE_VALUE: HANDLE = -1;
    pub const TH32CS_SNAPPROCESS: u32 = 0x00000002;
    pub const TH32CS_SNAPMODULE: u32 = 0x00000008;
    pub const TH32CS_SNAPMODULE32: u32 = 0x00000010;

    pub const PROCESS_CREATE_THREAD: u32 = 0x0002;
    pub const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    pub const PROCESS_VM_OPERATION: u32 = 0x0008;
    pub const PROCESS_VM_WRITE: u32 = 0x0020;
    pub const PROCESS_VM_READ: u32 = 0x0010;
    pub const PROCESS_ALL_ACCESS: u32 = 0x001F0FFF;

    pub const MEM_COMMIT: u32 = 0x00001000;
    pub const MEM_RESERVE: u32 = 0x00002000;
    pub const MEM_RELEASE: u32 = 0x00008000;
    pub const PAGE_READWRITE: u32 = 0x04;

    pub const DONT_RESOLVE_DLL_REFERENCES: u32 = 0x00000001;
    pub const WAIT_OBJECT_0: u32 = 0x00000000;

    #[repr(C)]
    pub struct PROCESSENTRY32W {
        pub dwSize: u32,
        pub cntUsage: u32,
        pub th32ProcessID: u32,
        pub th32DefaultHeapID: usize,
        pub th32ModuleID: u32,
        pub cntThreads: u32,
        pub th32ParentProcessID: u32,
        pub pcPriClassBase: i32,
        pub dwFlags: u32,
        pub szExeFile: [u16; 260],
    }

    #[repr(C)]
    pub struct MODULEENTRY32W {
        pub dwSize: u32,
        pub th32ModuleID: u32,
        pub th32ProcessID: u32,
        pub GlblcntUsage: u32,
        pub ProccntUsage: u32,
        pub modBaseAddr: *mut u8,
        pub modBaseSize: u32,
        pub hModule: HMODULE,
        pub szModule: [u16; 256],
        pub szExePath: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> HANDLE;
        pub fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        pub fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
        pub fn Module32FirstW(hSnapshot: HANDLE, lpme: *mut MODULEENTRY32W) -> BOOL;
        pub fn Module32NextW(hSnapshot: HANDLE, lpme: *mut MODULEENTRY32W) -> BOOL;
        pub fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: BOOL, dwProcessId: u32) -> HANDLE;
        pub fn VirtualAllocEx(
            hProcess: HANDLE,
            lpAddress: *const c_void,
            dwSize: usize,
            flAllocationType: u32,
            flProtect: u32,
        ) -> *mut c_void;
        pub fn WriteProcessMemory(
            hProcess: HANDLE,
            lpBaseAddress: *mut c_void,
            lpBuffer: *const c_void,
            nSize: usize,
            lpNumberOfBytesWritten: *mut usize,
        ) -> BOOL;
        pub fn GetModuleHandleW(lpModuleName: *const u16) -> HMODULE;
        pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const i8) -> *const c_void;
        pub fn CreateRemoteThread(
            hProcess: HANDLE,
            lpThreadAttributes: *const c_void,
            dwStackSize: usize,
            lpStartAddress: *const c_void,
            lpParameter: *const c_void,
            dwCreationFlags: u32,
            lpThreadId: *mut u32,
        ) -> HANDLE;
        pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: u32) -> u32;
        pub fn GetExitCodeThread(hThread: HANDLE, lpExitCode: *mut u32) -> BOOL;
        pub fn VirtualFreeEx(hProcess: HANDLE, lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> BOOL;
        pub fn CloseHandle(hObject: isize) -> BOOL;
        pub fn LoadLibraryExW(lpLibFileName: *const u16, hFile: HANDLE, dwFlags: u32) -> HMODULE;
        pub fn FreeLibrary(hLibModule: isize) -> BOOL;
        pub fn GetLastError() -> u32;
    }
}

pub struct ObsInjector;

impl ObsInjector {
    /// Converts a Rust string or OsStr to null-terminated UTF-16
    fn to_wide_null<S: AsRef<OsStr>>(s: S) -> Vec<u16> {
        s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Finds the running `obs64.exe` (or fallback `obs32.exe`) process ID
    pub fn find_obs_process() -> Option<u32> {
        #[cfg(windows)]
        unsafe {
            use win32::*;
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                cntUsage: 0,
                th32ProcessID: 0,
                th32DefaultHeapID: 0,
                th32ModuleID: 0,
                cntThreads: 0,
                th32ParentProcessID: 0,
                pcPriClassBase: 0,
                dwFlags: 0,
                szExeFile: [0; 260],
            };

            let mut found_pid = None;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..name_len]).to_lowercase();

                    if exe_name == "obs64.exe" || exe_name == "obs.exe" {
                        found_pid = Some(entry.th32ProcessID);
                        break;
                    }

                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            found_pid
        }

        #[cfg(not(windows))]
        None
    }

    /// Checks if a module matching `dll_name_substr` is already loaded in the target process
    pub fn find_module_base(pid: u32, dll_name_substr: &str) -> Option<*mut u8> {
        #[cfg(windows)]
        unsafe {
            use win32::*;
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry = MODULEENTRY32W {
                dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
                th32ModuleID: 0,
                th32ProcessID: 0,
                GlblcntUsage: 0,
                ProccntUsage: 0,
                modBaseAddr: std::ptr::null_mut(),
                modBaseSize: 0,
                hModule: 0,
                szModule: [0; 256],
                szExePath: [0; 260],
            };

            let mut found_base = None;
            if Module32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let name_len = entry.szModule.iter().position(|&c| c == 0).unwrap_or(entry.szModule.len());
                    let mod_name = String::from_utf16_lossy(&entry.szModule[..name_len]).to_lowercase();

                    if mod_name.contains(&dll_name_substr.to_lowercase()) {
                        found_base = Some(entry.modBaseAddr);
                        break;
                    }

                    if Module32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            found_base
        }

        #[cfg(not(windows))]
        None
    }

    /// Performs full dynamic injection into `obs64.exe` and calls `obs_module_load()`
    pub fn inject_into_obs(dll_path: &Path) -> Result<String, String> {
        if !dll_path.exists() {
            return Err(format!("DLL file for injection not found: {:?}", dll_path));
        }

        let pid = match Self::find_obs_process() {
            Some(p) => p,
            None => return Err("OBS Studio process (obs64.exe) is not running".to_string()),
        };

        println!("[Injector] Target OBS Studio process found: PID {}", pid);

        #[cfg(windows)]
        unsafe {
            use win32::*;

            // 1. Check if already injected
            if let Some(existing_base) = Self::find_module_base(pid, "obs-blewred") {
                println!("[Injector] obs-blewred.dll already loaded at {:p} in PID {}. Re-invoking obs_module_load...", existing_base, pid);
                let _ = Self::invoke_obs_module_load(pid, existing_base, dll_path);
                return Ok(format!("Plugin already loaded in OBS (PID: {}, Base: {:p})", pid, existing_base));
            }

            // 2. Open target OBS process
            let desired_access = PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ;

            let h_process = OpenProcess(desired_access, 0, pid);
            if h_process == 0 {
                let err = GetLastError();
                if err == 5 {
                    return Err(format!(
                        "Access denied (PID {}). OBS Studio is running as Administrator. Please launch blewred as Administrator for DLL injection.",
                        pid
                    ));
                }
                return Err(format!("Failed to open OBS process (PID {}): Win32 error code {}", pid, err));
            }

            let wide_path = Self::to_wide_null(dll_path.as_os_str());
            let path_bytes_len = wide_path.len() * std::mem::size_of::<u16>();

            // 3. Allocate memory in target process
            let remote_mem = VirtualAllocEx(
                h_process,
                std::ptr::null(),
                path_bytes_len,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            );

            if remote_mem.is_null() {
                let err = GetLastError();
                CloseHandle(h_process);
                return Err(format!("Failed to allocate memory in OBS process (PID {}): Win32 {}", pid, err));
            }

            // 4. Write DLL full path into remote process memory
            let mut written = 0;
            let write_ok = WriteProcessMemory(
                h_process,
                remote_mem,
                wide_path.as_ptr() as *const c_void,
                path_bytes_len,
                &mut written,
            );

            if write_ok == 0 {
                let err = GetLastError();
                VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
                CloseHandle(h_process);
                return Err(format!("Failed to write DLL path into OBS memory: Win32 {}", err));
            }

            // 5. Get address of LoadLibraryW in kernel32.dll
            let kernel32_wide = Self::to_wide_null("kernel32.dll");
            let h_kernel32 = GetModuleHandleW(kernel32_wide.as_ptr());
            if h_kernel32 == 0 {
                VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
                CloseHandle(h_process);
                return Err("Failed to get handle for kernel32.dll".to_string());
            }

            let load_lib_name = std::ffi::CString::new("LoadLibraryW").unwrap();
            let p_load_library = GetProcAddress(h_kernel32, load_lib_name.as_ptr());
            if p_load_library.is_null() {
                VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
                CloseHandle(h_process);
                return Err("Failed to find LoadLibraryW address".to_string());
            }

            // 6. Create Remote Thread in OBS to execute LoadLibraryW(remote_mem)
            let mut thread_id = 0;
            let h_thread = CreateRemoteThread(
                h_process,
                std::ptr::null(),
                0,
                p_load_library,
                remote_mem,
                0,
                &mut thread_id,
            );

            if h_thread == 0 {
                let err = GetLastError();
                VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);
                CloseHandle(h_process);
                return Err(format!("Failed to create remote thread for LoadLibraryW in OBS: Win32 {}", err));
            }

            // Wait up to 5 seconds for DLL load
            let wait_res = WaitForSingleObject(h_thread, 5000);
            let mut thread_exit_code = 0;
            GetExitCodeThread(h_thread, &mut thread_exit_code);

            CloseHandle(h_thread);
            VirtualFreeEx(h_process, remote_mem, 0, MEM_RELEASE);

            if wait_res != WAIT_OBJECT_0 {
                CloseHandle(h_process);
                return Err("Timed out waiting for DLL to load in OBS Studio".to_string());
            }

            println!("[Injector] LoadLibraryW executed in OBS (Thread Exit Code: {:#X})", thread_exit_code);

            // 7. Verify module base address in target process
            // Small pause for loader initialization
            std::thread::sleep(std::time::Duration::from_millis(150));

            let mod_base = match Self::find_module_base(pid, "obs-blewred") {
                Some(b) => b,
                None => {
                    CloseHandle(h_process);
                    return Err("DLL loaded, but obs-blewred module was not found in OBS module list".to_string());
                }
            };

            println!("[Injector] obs-blewred.dll successfully mapped in OBS at base: {:p}", mod_base);

            // 8. Call `obs_module_load()` directly in OBS process to register filter source
            let load_call_res = Self::invoke_obs_module_load(pid, mod_base, dll_path);
            CloseHandle(h_process);

            match load_call_res {
                Ok(_) => {
                    println!("[Injector] SUCCESS: obs-blewred.dll fully injected and registered in OBS Studio!");
                    Ok(format!("obs-blewred successfully injected into OBS Studio (PID {}, Base: {:p})", pid, mod_base))
                }
                Err(e) => {
                    println!("[Injector] DLL injected, but obs_module_load notice: {}", e);
                    Ok(format!("Plugin injected into OBS (PID {}), base address: {:p}", pid, mod_base))
                }
            }
        }

        #[cfg(not(windows))]
        Err("DLL injection is only supported on Windows".to_string())
    }

    /// Finds the RVA of `obs_module_load` in the local DLL and invokes it inside the target OBS process
    #[cfg(windows)]
    unsafe fn invoke_obs_module_load(pid: u32, remote_base: *mut u8, dll_path: &Path) -> Result<(), String> {
        use win32::*;

        let wide_path = Self::to_wide_null(dll_path.as_os_str());
        let h_local = LoadLibraryExW(wide_path.as_ptr(), 0, DONT_RESOLVE_DLL_REFERENCES);
        if h_local == 0 {
            return Err(format!("Failed to read local obs_module_load export: Win32 {}", GetLastError()));
        }

        let sym_name = std::ffi::CString::new("obs_module_load").unwrap();
        let p_local_fn = GetProcAddress(h_local, sym_name.as_ptr());
        if p_local_fn.is_null() {
            FreeLibrary(h_local);
            return Err("Export obs_module_load not found in DLL".to_string());
        }

        let rva = (p_local_fn as usize) - (h_local as usize);
        FreeLibrary(h_local);

        let remote_fn_addr = (remote_base as usize + rva) as *const c_void;
        println!("[Injector] Calculated remote obs_module_load address: {:p} (RVA: 0x{:X})", remote_fn_addr, rva);

        let desired_access = PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION;
        let h_process = OpenProcess(desired_access, 0, pid);
        if h_process == 0 {
            return Err(format!("OpenProcess failed for calling obs_module_load: Win32 {}", GetLastError()));
        }

        let mut thread_id = 0;
        let h_thread = CreateRemoteThread(
            h_process,
            std::ptr::null(),
            0,
            remote_fn_addr,
            std::ptr::null(),
            0,
            &mut thread_id,
        );

        if h_thread == 0 {
            let err = GetLastError();
            CloseHandle(h_process);
            return Err(format!("Failed to invoke remote obs_module_load: Win32 {}", err));
        }

        WaitForSingleObject(h_thread, 3000);
        CloseHandle(h_thread);
        CloseHandle(h_process);

        println!("[Injector] remote obs_module_load invoked successfully.");
        Ok(())
    }
}
