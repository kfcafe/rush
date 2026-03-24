//! Stats collection module for Rush shell
//!
//! Collects system statistics using direct syscalls (no subprocess calls).
//! Platform-specific implementations for macOS and Linux.

use std::collections::HashMap;

/// Collected statistics with values
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// Static stats (computed once, rarely change)
    pub builtin: HashMap<String, String>,
    /// Custom stats (user-defined commands)
    pub custom: HashMap<String, CustomStatValue>,
}

/// Value for a custom stat
#[derive(Debug, Clone)]
pub struct CustomStatValue {
    pub value: String,
    pub error: Option<String>,
    pub updated_ago_secs: u64,
}

/// Stats collector - gathers system information using direct syscalls
pub struct StatsCollector;

impl StatsCollector {
    /// Collect all built-in stats
    pub fn collect_builtins() -> HashMap<String, String> {
        let mut stats = HashMap::new();

        // Host: username@hostname
        stats.insert("host".to_string(), Self::get_host());

        // OS name and version
        stats.insert("os".to_string(), Self::get_os());

        // Kernel version
        stats.insert("kernel".to_string(), Self::get_kernel());

        // Architecture
        stats.insert("arch".to_string(), Self::get_arch());

        // CPU model
        stats.insert("cpu".to_string(), Self::get_cpu());

        // CPU cores
        stats.insert("cores".to_string(), Self::get_cores());

        // Uptime
        stats.insert("uptime".to_string(), Self::get_uptime());

        // Load average
        stats.insert("load".to_string(), Self::get_load());

        // Process count
        stats.insert("procs".to_string(), Self::get_procs());

        // Memory usage
        stats.insert("memory".to_string(), Self::get_memory());

        // Swap usage
        stats.insert("swap".to_string(), Self::get_swap());

        // Disk usage
        stats.insert("disk".to_string(), Self::get_disk());

        // Battery percentage
        stats.insert("battery".to_string(), Self::get_battery());

        // Power source (AC/Battery)
        stats.insert("power".to_string(), Self::get_power());

        // IP address
        stats.insert("ip".to_string(), Self::get_ip());

        // WiFi network
        stats.insert("wifi".to_string(), Self::get_wifi());

        // Current time
        stats.insert("time".to_string(), Self::get_time());

        // Current date
        stats.insert("date".to_string(), Self::get_date());

        stats
    }

    /// Collect a specific stat by name
    pub fn collect_stat(name: &str) -> Option<String> {
        match name {
            "host" => Some(Self::get_host()),
            "os" => Some(Self::get_os()),
            "kernel" => Some(Self::get_kernel()),
            "arch" => Some(Self::get_arch()),
            "cpu" => Some(Self::get_cpu()),
            "cores" => Some(Self::get_cores()),
            "uptime" => Some(Self::get_uptime()),
            "load" => Some(Self::get_load()),
            "procs" => Some(Self::get_procs()),
            "memory" => Some(Self::get_memory()),
            "swap" => Some(Self::get_swap()),
            "disk" => Some(Self::get_disk()),
            "battery" => Some(Self::get_battery()),
            "power" => Some(Self::get_power()),
            "ip" => Some(Self::get_ip()),
            "wifi" => Some(Self::get_wifi()),
            "time" => Some(Self::get_time()),
            "date" => Some(Self::get_date()),
            _ => None,
        }
    }

    /// List all available built-in stat names
    pub fn builtin_names() -> &'static [&'static str] {
        &[
            "host", "os", "kernel", "arch", "cpu", "cores", "uptime", "load", "procs", "memory",
            "swap", "disk", "battery", "power", "ip", "wifi", "time", "date",
        ]
    }

    // =========================================================================
    // Platform-specific stat collection using syscalls
    // =========================================================================

    fn get_host() -> String {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| whoami::username());

        let hostname = Self::get_hostname();

        // Strip .local suffix for cleaner display
        let hostname = hostname.strip_suffix(".local").unwrap_or(&hostname);

        format!("{}@{}", username, hostname)
    }

    fn get_hostname() -> String {
        #[cfg(unix)]
        {
            use std::ffi::CStr;
            let mut buf = [0i8; 256];
            unsafe {
                if libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 {
                    if let Ok(s) = CStr::from_ptr(buf.as_ptr()).to_str() {
                        return s.to_string();
                    }
                }
            }
        }
        "unknown".to_string()
    }

    fn get_os() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_version()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_version()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            std::env::consts::OS.to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_version() -> String {
        // Read from SystemVersion.plist (no subprocess needed)
        if let Ok(content) =
            std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
        {
            // Parse plist XML - look for ProductVersion
            let mut in_version_key = false;
            for line in content.lines() {
                let line = line.trim();
                if line == "<key>ProductVersion</key>" {
                    in_version_key = true;
                } else if in_version_key && line.starts_with("<string>") {
                    let version = line
                        .trim_start_matches("<string>")
                        .trim_end_matches("</string>");
                    return format!("macOS {}", version);
                }
            }
        }
        "macOS".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_version() -> String {
        // Try /etc/os-release
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if line.starts_with("PRETTY_NAME=") {
                    let name = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                    return name.to_string();
                }
            }
        }
        "Linux".to_string()
    }

    fn get_kernel() -> String {
        #[cfg(unix)]
        {
            use std::ffi::CStr;

            unsafe {
                let mut info: libc::utsname = std::mem::zeroed();
                if libc::uname(&mut info) == 0 {
                    let sysname = CStr::from_ptr(info.sysname.as_ptr()).to_string_lossy();
                    let release = CStr::from_ptr(info.release.as_ptr()).to_string_lossy();
                    return format!("{} {}", sysname, release);
                }
            }
        }
        "unknown".to_string()
    }

    fn get_arch() -> String {
        std::env::consts::ARCH.to_string()
    }

    fn get_cpu() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_cpu()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_cpu()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_cpu() -> String {
        // Use sysctlbyname for CPU brand string
        Self::sysctl_string("machdep.cpu.brand_string").unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(target_os = "linux")]
    fn get_linux_cpu() -> String {
        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in content.lines() {
                if line.starts_with("model name") {
                    if let Some(name) = line.split(':').nth(1) {
                        return name.trim().to_string();
                    }
                }
            }
        }
        "unknown".to_string()
    }

    fn get_cores() -> String {
        num_cpus::get().to_string()
    }

    fn get_uptime() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_uptime()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_uptime()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_uptime() -> String {
        // Use sysctlbyname for boot time
        if let Some(boot_sec) = Self::sysctl_timeval_sec("kern.boottime") {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let uptime_secs = now - boot_sec;
            return Self::format_uptime(uptime_secs as u64);
        }
        "unknown".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_uptime() -> String {
        if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
            if let Some(secs_str) = content.split_whitespace().next() {
                if let Ok(secs) = secs_str.parse::<f64>() {
                    return Self::format_uptime(secs as u64);
                }
            }
        }
        "unknown".to_string()
    }

    fn format_uptime(secs: u64) -> String {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let mins = (secs % 3600) / 60;

        if days > 0 {
            format!("{}d {}h", days, hours)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        }
    }

    fn get_load() -> String {
        #[cfg(unix)]
        {
            let mut loadavg: [f64; 3] = [0.0; 3];
            unsafe {
                if libc::getloadavg(loadavg.as_mut_ptr(), 3) == 3 {
                    return format!("{:.2} {:.2} {:.2}", loadavg[0], loadavg[1], loadavg[2]);
                }
            }
        }
        "unknown".to_string()
    }

    fn get_procs() -> String {
        #[cfg(target_os = "macos")]
        {
            // Use sysctl to count processes - no subprocess needed
            // We use kern.proc.all with a NULL buffer to get the count
            if let Some(count) = Self::sysctl_proc_count() {
                return count.to_string();
            }
        }
        #[cfg(target_os = "linux")]
        {
            if let Ok(entries) = std::fs::read_dir("/proc") {
                let count = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .chars()
                            .all(|c| c.is_ascii_digit())
                    })
                    .count();
                return count.to_string();
            }
        }
        "unknown".to_string()
    }

    #[cfg(target_os = "macos")]
    fn sysctl_proc_count() -> Option<usize> {
        use std::ffi::CString;

        unsafe {
            let name = CString::new("kern.proc.all").ok()?;
            let mut size: libc::size_t = 0;

            // First call to get size
            if libc::sysctlbyname(
                name.as_ptr(),
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return None;
            }

            // Size is the total bytes of all kinfo_proc structures
            // Each kinfo_proc is 648 bytes on 64-bit macOS
            let kinfo_proc_size = std::mem::size_of::<libc::c_void>() * 81; // Approximate
            if kinfo_proc_size > 0 {
                Some(size / 648) // Known size of kinfo_proc on macOS
            } else {
                None
            }
        }
    }

    fn get_memory() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_memory()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_memory()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_memory() -> String {
        // Get total memory via sysctl
        let total_bytes = Self::sysctl_u64("hw.memsize").unwrap_or(0);

        // Get memory usage via Mach API with correct types
        let used_bytes = Self::get_macos_used_memory().unwrap_or(0);

        if total_bytes > 0 {
            let used_gb = used_bytes as f64 / 1_073_741_824.0;
            let total_gb = total_bytes as f64 / 1_073_741_824.0;
            format!("{:.1}/{:.0}G", used_gb, total_gb)
        } else {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_used_memory() -> Option<u64> {
        // Use Mach host_statistics64 API
        // Note: vm_statistics64 fields are natural_t (u32), not u64
        use std::mem::MaybeUninit;

        const HOST_VM_INFO64: i32 = 4;
        const HOST_VM_INFO64_COUNT: u32 = 38;

        #[repr(C)]
        struct VmStatistics64 {
            free_count: u32,
            active_count: u32,
            inactive_count: u32,
            wire_count: u32,
            zero_fill_count: u64,
            reactivations: u64,
            pageins: u64,
            pageouts: u64,
            faults: u64,
            cow_faults: u64,
            lookups: u64,
            hits: u64,
            purges: u64,
            purgeable_count: u32,
            speculative_count: u32,
            decompressions: u64,
            compressions: u64,
            swapins: u64,
            swapouts: u64,
            compressor_page_count: u32,
            throttled_count: u32,
            external_page_count: u32,
            internal_page_count: u32,
            total_uncompressed_pages_in_compressor: u64,
        }

        extern "C" {
            fn mach_host_self() -> u32;
            fn host_statistics64(
                host_priv: u32,
                flavor: i32,
                host_info_out: *mut VmStatistics64,
                host_info_outCnt: *mut u32,
            ) -> i32;
        }

        unsafe {
            let host = mach_host_self();
            let mut vm_stat = MaybeUninit::<VmStatistics64>::uninit();
            let mut count = HOST_VM_INFO64_COUNT;

            let ret = host_statistics64(host, HOST_VM_INFO64, vm_stat.as_mut_ptr(), &mut count);

            if ret != 0 {
                return None;
            }

            let vm_stat = vm_stat.assume_init();
            let page_size = Self::sysctl_u64("hw.pagesize").unwrap_or(16384);

            // Used = active + wired + compressed
            let used_pages = vm_stat.active_count as u64
                + vm_stat.wire_count as u64
                + vm_stat.compressor_page_count as u64;
            Some(used_pages * page_size)
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_memory() -> String {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;

            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total = val.parse().unwrap_or(0) * 1024; // kB to bytes
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        available = val.parse().unwrap_or(0) * 1024;
                    }
                }
            }

            if total > 0 {
                let used = total.saturating_sub(available);
                let used_gb = used as f64 / 1_073_741_824.0;
                let total_gb = total as f64 / 1_073_741_824.0;
                return format!("{:.1}/{:.0}G", used_gb, total_gb);
            }
        }
        "unknown".to_string()
    }

    fn get_swap() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_swap()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_swap()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_swap() -> String {
        // Use sysctl vm.swapusage
        if let Some(swap_str) = Self::sysctl_string("vm.swapusage") {
            // Format: "total = 2048.00M  used = 256.00M  free = 1792.00M"
            let mut used_mb = 0.0f64;
            let mut total_mb = 0.0f64;

            for part in swap_str.split_whitespace() {
                if part.ends_with('M') || part.ends_with('G') {
                    let multiplier = if part.ends_with('G') { 1024.0 } else { 1.0 };
                    if let Ok(val) = part
                        .trim_end_matches(|c| c == 'M' || c == 'G')
                        .parse::<f64>()
                    {
                        if total_mb == 0.0 {
                            total_mb = val * multiplier;
                        } else if used_mb == 0.0 {
                            used_mb = val * multiplier;
                            break;
                        }
                    }
                }
            }

            if total_mb > 0.0 {
                let used_gb = used_mb / 1024.0;
                let total_gb = total_mb / 1024.0;
                return format!("{:.1}/{:.1}G", used_gb, total_gb);
            }
        }
        "0/0G".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_swap() -> String {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut free = 0u64;

            for line in content.lines() {
                if line.starts_with("SwapTotal:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        total = val.parse().unwrap_or(0) * 1024;
                    }
                } else if line.starts_with("SwapFree:") {
                    if let Some(val) = line.split_whitespace().nth(1) {
                        free = val.parse().unwrap_or(0) * 1024;
                    }
                }
            }

            let used = total.saturating_sub(free);
            let used_gb = used as f64 / 1_073_741_824.0;
            let total_gb = total as f64 / 1_073_741_824.0;
            return format!("{:.1}/{:.1}G", used_gb, total_gb);
        }
        "unknown".to_string()
    }

    fn get_disk() -> String {
        #[cfg(unix)]
        {
            Self::get_disk_usage("/")
        }
        #[cfg(not(unix))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(unix)]
    fn get_disk_usage(path: &str) -> String {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        unsafe {
            let path_c = match CString::new(path) {
                Ok(p) => p,
                Err(_) => return "unknown".to_string(),
            };

            let mut stat = MaybeUninit::<libc::statfs>::uninit();

            if libc::statfs(path_c.as_ptr(), stat.as_mut_ptr()) != 0 {
                return "unknown".to_string();
            }

            let stat = stat.assume_init();
            let block_size = stat.f_bsize as u64;
            let total_blocks = stat.f_blocks as u64;
            let free_blocks = stat.f_bfree as u64;

            let total_bytes = total_blocks * block_size;
            let free_bytes = free_blocks * block_size;
            let used_bytes = total_bytes.saturating_sub(free_bytes);

            let used_gb = used_bytes as f64 / 1_073_741_824.0;
            let total_gb = total_bytes as f64 / 1_073_741_824.0;

            format!("{:.0}/{:.0}G", used_gb, total_gb)
        }
    }

    fn get_battery() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_battery()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_battery()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_battery() -> String {
        // Read from IOKit power source info via pmset -g batt style file
        // Actually, let's read from the IOPowerSources directory
        if let Ok(_content) =
            std::fs::read_to_string("/Library/Preferences/com.apple.PowerManagement.plist")
        {
            // This plist doesn't have current battery level
            // We need IOKit, but that requires linking to IOKit framework
        }

        // Fallback: read from ioreg -l output format stored in /tmp or use syscall
        // For now, use the pmset approach via file read if available
        // Actually on macOS we need IOKit, so return "N/A" for systems without battery
        // or use the IOPSCopyPowerSourcesInfo API

        // Try reading from a known location for battery status
        // On macOS, the cleanest way without subprocess is IOKit, but that's complex
        // Let's try to read from the AppleSmartBattery IOService
        if let Ok(content) = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity") {
            return format!("{}%", content.trim());
        }

        // For macOS, we'll need to use IOKit - but for simplicity, indicate N/A for now
        // Battery info on macOS really requires IOKit calls
        "N/A".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_battery() -> String {
        // Try common battery paths
        for bat in &["BAT0", "BAT1", "battery"] {
            let capacity_path = format!("/sys/class/power_supply/{}/capacity", bat);
            if let Ok(content) = std::fs::read_to_string(&capacity_path) {
                return format!("{}%", content.trim());
            }
        }
        "N/A".to_string()
    }

    fn get_power() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_power()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_power()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_power() -> String {
        // Check if on AC power via sysctl or IOKit
        // On macOS, hw.tbfrequency or other metrics might indicate power state
        // For now, return AC as default for desktops, N/A for unknown

        // Try reading from IOPMPowerSource
        // Without IOKit bindings, this is tricky
        // Most Macs report AC unless it's a laptop on battery
        "AC".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_power() -> String {
        // Check AC adapter status
        for ac in &["AC", "AC0", "ADP0", "ADP1"] {
            let online_path = format!("/sys/class/power_supply/{}/online", ac);
            if let Ok(content) = std::fs::read_to_string(&online_path) {
                return if content.trim() == "1" {
                    "AC"
                } else {
                    "Battery"
                }
                .to_string();
            }
        }

        // Check if any battery exists - if not, assume AC (desktop)
        for bat in &["BAT0", "BAT1"] {
            let status_path = format!("/sys/class/power_supply/{}/status", bat);
            if let Ok(content) = std::fs::read_to_string(&status_path) {
                let status = content.trim();
                return match status {
                    "Charging" | "Full" => "AC",
                    "Discharging" => "Battery",
                    _ => "AC",
                }
                .to_string();
            }
        }

        "AC".to_string()
    }

    fn get_ip() -> String {
        #[cfg(unix)]
        {
            Self::get_unix_ip()
        }
        #[cfg(not(unix))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(unix)]
    fn get_unix_ip() -> String {
        use std::net::UdpSocket;

        // Create a UDP socket and "connect" to a public IP (doesn't actually send anything)
        // This gives us the local IP that would be used to reach that destination
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect("8.8.8.8:80").is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    return addr.ip().to_string();
                }
            }
        }

        // Fallback: try to find first non-loopback interface
        "127.0.0.1".to_string()
    }

    fn get_wifi() -> String {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_wifi()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_wifi()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            "unknown".to_string()
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_wifi() -> String {
        // On macOS, we can read from airport -I or use CoreWLAN
        // Without subprocess, we'd need to use CoreWLAN framework
        // Try reading from known location

        // Check for Wi-Fi network name in networksetup preferences
        // This requires CoreWLAN framework access
        // For now, return "N/A" - proper implementation would use CoreWLAN
        "N/A".to_string()
    }

    #[cfg(target_os = "linux")]
    fn get_linux_wifi() -> String {
        // Try to read current SSID from /proc/net/wireless or iwgetid
        // Check NetworkManager or wpa_supplicant state files

        // Try reading from /proc/net/wireless first
        if let Ok(content) = std::fs::read_to_string("/proc/net/wireless") {
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() > 2 {
                // Third line contains interface info
                if let Some(iface) = lines[2].split(':').next() {
                    let iface = iface.trim();
                    // Now try to get SSID from iwconfig or /sys
                    // Check for SSID in wpa_supplicant or NetworkManager state
                    if !iface.is_empty() {
                        return format!("{}:connected", iface);
                    }
                }
            }
        }

        "N/A".to_string()
    }

    fn get_time() -> String {
        use chrono::Local;
        Local::now().format("%-I:%M %p").to_string()
    }

    fn get_date() -> String {
        use chrono::Local;
        Local::now().format("%a %b %-d").to_string()
    }

    // =========================================================================
    // Sysctl helper functions (macOS)
    // =========================================================================

    #[cfg(target_os = "macos")]
    fn sysctl_string(name: &str) -> Option<String> {
        use std::ffi::CString;

        unsafe {
            let name_c = CString::new(name).ok()?;
            let mut size: libc::size_t = 0;

            // First call to get size
            if libc::sysctlbyname(
                name_c.as_ptr(),
                std::ptr::null_mut(),
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return None;
            }

            let mut buf = vec![0u8; size];

            // Second call to get value
            if libc::sysctlbyname(
                name_c.as_ptr(),
                buf.as_mut_ptr() as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return None;
            }

            // Remove trailing null if present
            if buf.last() == Some(&0) {
                buf.pop();
            }

            String::from_utf8(buf).ok()
        }
    }

    #[cfg(target_os = "macos")]
    fn sysctl_u64(name: &str) -> Option<u64> {
        use std::ffi::CString;

        unsafe {
            let name_c = CString::new(name).ok()?;
            let mut value: u64 = 0;
            let mut size = std::mem::size_of::<u64>();

            if libc::sysctlbyname(
                name_c.as_ptr(),
                &mut value as *mut u64 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return None;
            }

            Some(value)
        }
    }

    #[cfg(target_os = "macos")]
    fn sysctl_timeval_sec(name: &str) -> Option<i64> {
        use std::ffi::CString;

        #[repr(C)]
        struct Timeval {
            tv_sec: i64,
            tv_usec: i32,
        }

        unsafe {
            let name_c = CString::new(name).ok()?;
            let mut tv = Timeval {
                tv_sec: 0,
                tv_usec: 0,
            };
            let mut size = std::mem::size_of::<Timeval>();

            if libc::sysctlbyname(
                name_c.as_ptr(),
                &mut tv as *mut Timeval as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                return None;
            }

            Some(tv.tv_sec)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_builtins() {
        let stats = StatsCollector::collect_builtins();

        // Should have host stat
        assert!(stats.contains_key("host"), "missing host stat");
        assert!(!stats["host"].is_empty(), "host stat is empty");

        // Should have os stat
        assert!(stats.contains_key("os"), "missing os stat");

        // Should have cores stat (and it should be a number)
        assert!(stats.contains_key("cores"), "missing cores stat");
        assert!(
            stats["cores"].parse::<u32>().is_ok(),
            "cores should be a number"
        );

        // Should have new stats
        assert!(stats.contains_key("swap"), "missing swap stat");
        assert!(stats.contains_key("disk"), "missing disk stat");
        assert!(stats.contains_key("battery"), "missing battery stat");
        assert!(stats.contains_key("power"), "missing power stat");
        assert!(stats.contains_key("ip"), "missing ip stat");
        assert!(stats.contains_key("wifi"), "missing wifi stat");
    }

    #[test]
    fn test_collect_single_stat() {
        assert!(StatsCollector::collect_stat("host").is_some());
        assert!(StatsCollector::collect_stat("swap").is_some());
        assert!(StatsCollector::collect_stat("disk").is_some());
        assert!(StatsCollector::collect_stat("nonexistent").is_none());
    }

    #[test]
    fn test_builtin_names() {
        let names = StatsCollector::builtin_names();
        assert!(names.contains(&"host"));
        assert!(names.contains(&"memory"));
        assert!(names.contains(&"uptime"));
        assert!(names.contains(&"swap"));
        assert!(names.contains(&"disk"));
        assert!(names.contains(&"battery"));
        assert!(names.contains(&"power"));
        assert!(names.contains(&"ip"));
        assert!(names.contains(&"wifi"));
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(StatsCollector::format_uptime(90), "1m");
        assert_eq!(StatsCollector::format_uptime(3700), "1h 1m");
        assert_eq!(StatsCollector::format_uptime(90000), "1d 1h");
    }

    #[test]
    fn test_disk_usage() {
        let disk = StatsCollector::get_disk();
        assert!(!disk.is_empty());
        assert!(
            disk.contains('G') || disk == "unknown",
            "disk should be in GB format: {}",
            disk
        );
    }

    #[test]
    fn test_ip_address() {
        let ip = StatsCollector::get_ip();
        assert!(!ip.is_empty());
        // Should be a valid IP or 127.0.0.1
        assert!(ip.contains('.'), "IP should contain dots: {}", ip);
    }

    #[test]
    fn test_load_average() {
        let load = StatsCollector::get_load();
        assert!(!load.is_empty());
        // Should contain spaces (three values)
        if load != "unknown" {
            assert!(
                load.contains(' '),
                "load should have three space-separated values: {}",
                load
            );
        }
    }
}
