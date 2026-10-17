pub mod setup;

use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use tracing::debug;

/// Detected GPU device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuDevice {
    Cuda {
        index: usize,
        name: String,
        vram_total: u64,
        vram_free: u64,
    },
    Rocm {
        index: usize,
        name: String,
        vram_total: u64,
        vram_free: u64,
        /// true if this is an APU sharing system memory
        is_apu: bool,
    },
    Metal {
        unified_memory: u64,
    },
    Cpu,
}

impl GpuDevice {
    pub fn device_name(&self) -> String {
        match self {
            GpuDevice::Cuda { name, index, .. } => format!("CUDA:{index} ({name})"),
            GpuDevice::Rocm {
                name,
                index,
                is_apu,
                ..
            } => {
                let suffix = if *is_apu { " [APU]" } else { "" };
                format!("ROCm:{index} ({name}{suffix})")
            }
            GpuDevice::Metal { unified_memory } => {
                format!(
                    "Metal ({}GB unified)",
                    unified_memory / (1024 * 1024 * 1024)
                )
            }
            GpuDevice::Cpu => "CPU".to_string(),
        }
    }

    pub fn vram_free(&self) -> u64 {
        match self {
            GpuDevice::Cuda { vram_free, .. } => *vram_free,
            GpuDevice::Rocm { vram_free, .. } => *vram_free,
            GpuDevice::Metal { unified_memory } => *unified_memory / 2,
            GpuDevice::Cpu => 0,
        }
    }

    pub fn vram_total(&self) -> u64 {
        match self {
            GpuDevice::Cuda { vram_total, .. } => *vram_total,
            GpuDevice::Rocm { vram_total, .. } => *vram_total,
            GpuDevice::Metal { unified_memory } => *unified_memory,
            GpuDevice::Cpu => 0,
        }
    }

    pub fn is_gpu(&self) -> bool {
        !matches!(self, GpuDevice::Cpu)
    }

    pub fn vendor(&self) -> &'static str {
        match self {
            GpuDevice::Cuda { .. } => "NVIDIA",
            GpuDevice::Rocm { .. } => "AMD",
            GpuDevice::Metal { .. } => "Apple",
            GpuDevice::Cpu => "CPU",
        }
    }
}

// --- Detection ---

/// Detect all available GPU devices on the system.
/// Uses sysfs on Linux for zero-dependency detection, falling back to
/// platform-specific APIs when available.
pub fn detect_devices() -> Vec<GpuDevice> {
    let mut devices = Vec::new();

    // Linux: scan sysfs for all GPUs (NVIDIA + AMD) without external deps
    #[cfg(target_os = "linux")]
    {
        devices.extend(detect_linux_gpus());
    }

    // macOS: Metal detection
    #[cfg(target_os = "macos")]
    {
        if let Some(metal_device) = detect_metal_device() {
            devices.push(metal_device);
        }
    }

    // Windows: detect display adapters via WMI/CIM
    #[cfg(target_os = "windows")]
    {
        devices.extend(detect_windows_gpus());
    }

    // CUDA via NVML (if compiled with cuda feature, supplements sysfs data)
    #[cfg(feature = "cuda")]
    {
        if let Ok(cuda_devices) = detect_cuda_nvml() {
            // Replace any previously detected NVIDIA entries with richer NVML data.
            devices.retain(|d| !matches!(d, GpuDevice::Cuda { .. }));
            devices.extend(cuda_devices);
        }
    }

    // Always include CPU as fallback
    devices.push(GpuDevice::Cpu);

    devices
}

/// Select the best available device
pub fn best_device() -> GpuDevice {
    let devices = detect_devices();
    devices
        .into_iter()
        .find(|d| d.is_gpu())
        .unwrap_or(GpuDevice::Cpu)
}

// --- Linux sysfs detection (no external dependencies) ---

#[cfg(target_os = "linux")]
fn detect_linux_gpus() -> Vec<GpuDevice> {
    let mut devices = Vec::new();
    let drm_path = Path::new("/sys/class/drm");

    let Ok(entries) = fs::read_dir(drm_path) else {
        return devices;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Only look at cardN entries (skip renderD128, etc.)
        if !name_str.starts_with("card") || name_str.contains('-') {
            continue;
        }

        let card_path = entry.path();
        let device_path = card_path.join("device");

        let vendor = read_sysfs_hex(&device_path.join("vendor")).unwrap_or(0);

        match vendor {
            0x10de => {
                // NVIDIA
                if let Some(dev) = detect_nvidia_sysfs(
                    &device_path,
                    devices
                        .iter()
                        .filter(|d| matches!(d, GpuDevice::Cuda { .. }))
                        .count(),
                ) {
                    devices.push(dev);
                }
            }
            0x1002 => {
                // AMD
                if let Some(dev) = detect_amd_sysfs(
                    &device_path,
                    devices
                        .iter()
                        .filter(|d| matches!(d, GpuDevice::Rocm { .. }))
                        .count(),
                ) {
                    devices.push(dev);
                }
            }
            _ => {
                debug!("Unknown GPU vendor: {:#06x}", vendor);
            }
        }
    }

    devices
}

#[cfg(target_os = "linux")]
fn detect_nvidia_sysfs(device_path: &Path, index: usize) -> Option<GpuDevice> {
    // Try to read GPU name from various sysfs locations
    let name = read_sysfs_string(&device_path.join("label"))
        .or_else(|| {
            // Parse PCI device ID and map to a name via lspci-style lookup
            let pci_slot = read_sysfs_string(&device_path.join("uevent")).and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("PCI_SLOT_NAME="))
                    .map(|l| l.trim_start_matches("PCI_SLOT_NAME=").to_string())
            });
            pci_slot.and_then(|slot| pci_device_name(&slot))
        })
        .unwrap_or_else(|| "NVIDIA GPU".to_string());

    // VRAM info — NVIDIA exposes this via their driver sysfs
    let vram_total = read_sysfs_u64(&device_path.join("mem_info_vram_total")).unwrap_or(0);
    let vram_used = read_sysfs_u64(&device_path.join("mem_info_vram_used")).unwrap_or(0);
    let vram_free = vram_total.saturating_sub(vram_used);

    Some(GpuDevice::Cuda {
        index,
        name,
        vram_total,
        vram_free,
    })
}

#[cfg(target_os = "linux")]
fn detect_amd_sysfs(device_path: &Path, index: usize) -> Option<GpuDevice> {
    // Get GPU name
    let name = read_sysfs_string(&device_path.join("label"))
        .or_else(|| {
            let pci_slot = read_sysfs_string(&device_path.join("uevent")).and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("PCI_SLOT_NAME="))
                    .map(|l| l.trim_start_matches("PCI_SLOT_NAME=").to_string())
            });
            pci_slot.and_then(|slot| pci_device_name(&slot))
        })
        .unwrap_or_else(|| "AMD GPU".to_string());

    // VRAM — AMD amdgpu driver exposes mem_info_vram_total
    let vram_total = read_sysfs_u64(&device_path.join("mem_info_vram_total")).unwrap_or(0);
    let vram_used = read_sysfs_u64(&device_path.join("mem_info_vram_used")).unwrap_or(0);

    // If no dedicated VRAM sysfs, check if it's an APU (shared system memory)
    let is_apu = vram_total == 0 || {
        // APUs typically report VRAM == system RAM or have specific flags
        let total_system = read_sysfs_string(&Path::new("/proc/meminfo"))
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("MemTotal:"))
                    .and_then(|l| {
                        l.split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse::<u64>().ok())
                    })
            })
            .unwrap_or(0)
            * 1024; // meminfo is in kB
                    // APU: VRAM is carved from system memory, typically matches or exceeds 50% of system RAM
        vram_total > total_system / 2
    };

    // For APUs with no VRAM reported, use system memory as the pool
    let (effective_total, effective_free) = if vram_total > 0 {
        (vram_total, vram_total.saturating_sub(vram_used))
    } else {
        let sys_total = system_memory_total();
        let sys_free = system_memory_available();
        (sys_total, sys_free)
    };

    // Check if compute is available (/dev/kfd present = ROCm kernel driver)
    let has_compute = Path::new("/dev/kfd").exists();
    if !has_compute {
        debug!("AMD GPU found but /dev/kfd not present — no compute support");
    }

    Some(GpuDevice::Rocm {
        index,
        name,
        vram_total: effective_total,
        vram_free: effective_free,
        is_apu,
    })
}

// --- Windows detection ---

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct WindowsVideoController {
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "AdapterRAM")]
    pub adapter_ram: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct WindowsMemoryInfo {
    #[serde(rename = "FreePhysicalMemory")]
    pub free_physical_memory_kb: Option<u64>,
    #[serde(rename = "TotalVisibleMemorySize")]
    pub total_visible_memory_kb: Option<u64>,
}

#[cfg(target_os = "windows")]
fn detect_windows_gpus() -> Vec<GpuDevice> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Select-Object Name,AdapterRAM | ConvertTo-Json -Compress",
        ])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let json = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let (sys_total, sys_free) = windows_system_memory_bytes();

    build_windows_gpu_devices(&json, sys_total, sys_free, Some(&query_nvidia_memory))
}

#[allow(dead_code)]
pub(crate) fn parse_windows_video_controllers(json: &str) -> Vec<WindowsVideoController> {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Vec::new();
    }
    if let Ok(arr) = serde_json::from_str::<Vec<WindowsVideoController>>(trimmed) {
        return arr;
    }
    if let Ok(one) = serde_json::from_str::<WindowsVideoController>(trimmed) {
        return vec![one];
    }
    Vec::new()
}

#[allow(dead_code)]
pub(crate) fn build_windows_gpu_devices(
    json: &str,
    sys_total: u64,
    sys_free: u64,
    nvidia_query: Option<&dyn Fn(usize, u64) -> (u64, u64)>,
) -> Vec<GpuDevice> {
    let mut devices = Vec::new();
    let controllers = parse_windows_video_controllers(json);

    let mut cuda_index = 0usize;
    let mut rocm_index = 0usize;

    for vc in controllers {
        let name = vc.name.unwrap_or_default().trim().to_string();
        if name.is_empty() {
            continue;
        }

        let lower = name.to_ascii_lowercase();
        if lower.contains("microsoft basic") {
            continue;
        }

        let adapter_ram = vc.adapter_ram.unwrap_or(0);

        if lower.contains("nvidia") {
            let (vram_total, vram_free) = match nvidia_query {
                Some(q) => q(cuda_index, adapter_ram),
                None => (adapter_ram, adapter_ram.saturating_mul(8) / 10),
            };
            devices.push(GpuDevice::Cuda {
                index: cuda_index,
                name,
                vram_total,
                vram_free,
            });
            cuda_index += 1;
            continue;
        }

        if lower.contains("amd")
            || lower.contains("radeon")
            || lower.contains("intel")
            || lower.contains("arc")
        {
            let is_apu = lower.contains("integrated")
                || lower.contains("radeon(tm)")
                || (adapter_ram > 0 && adapter_ram <= 8 * 1024 * 1024 * 1024);

            let vram_total = if adapter_ram > 0 {
                adapter_ram
            } else if is_apu {
                sys_total
            } else {
                0
            };

            let mut vram_free = if is_apu {
                sys_free
            } else if vram_total > 0 {
                // Rough fallback for dedicated adapters when free VRAM is unavailable.
                vram_total.saturating_mul(8) / 10
            } else {
                0
            };

            if vram_total > 0 {
                vram_free = vram_free.min(vram_total);
            }

            devices.push(GpuDevice::Rocm {
                index: rocm_index,
                name,
                vram_total,
                vram_free,
                is_apu,
            });
            rocm_index += 1;
        }
    }

    devices
}

#[allow(dead_code)]
pub(crate) fn parse_windows_memory_json(json: &str) -> (u64, u64) {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return (0, 0);
    }

    let Ok(mem) = serde_json::from_str::<WindowsMemoryInfo>(trimmed) else {
        return (0, 0);
    };

    let total = mem
        .total_visible_memory_kb
        .unwrap_or(0)
        .saturating_mul(1024);
    let free = mem
        .free_physical_memory_kb
        .unwrap_or(0)
        .saturating_mul(1024);
    (total, free.min(total))
}

#[cfg(target_os = "windows")]
fn windows_system_memory_bytes() -> (u64, u64) {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_OperatingSystem | Select-Object TotalVisibleMemorySize,FreePhysicalMemory | ConvertTo-Json -Compress",
        ])
        .output();

    let Ok(output) = output else {
        return (0, 0);
    };
    if !output.status.success() {
        return (0, 0);
    }

    let json = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_windows_memory_json(&json)
}

#[cfg(target_os = "windows")]
fn query_nvidia_memory(index: usize, fallback_total: u64) -> (u64, u64) {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.total,memory.free",
            "--format=csv,noheader,nounits",
            "-i",
            &index.to_string(),
        ])
        .output();

    let Ok(output) = output else {
        let total = fallback_total;
        let free = total.saturating_mul(8) / 10;
        return (total, free);
    };
    if !output.status.success() {
        let total = fallback_total;
        let free = total.saturating_mul(8) / 10;
        return (total, free);
    };

    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let parts: Vec<&str> = line.split(',').map(|p| p.trim()).collect();
    if parts.len() < 2 {
        let total = fallback_total;
        let free = total.saturating_mul(8) / 10;
        return (total, free);
    }

    let total_mib = parts[0].parse::<u64>().unwrap_or(0);
    let free_mib = parts[1].parse::<u64>().unwrap_or(0);
    let total = total_mib.saturating_mul(1024 * 1024);
    let free = free_mib.saturating_mul(1024 * 1024).min(total);
    (total, free)
}

// --- Cross-platform system memory ---

/// Return `(total_ram_bytes, available_ram_bytes)` across Linux, macOS, and Windows.
pub fn system_memory_bytes() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        return parse_proc_meminfo(&content);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_system_memory_bytes();
    }
    #[cfg(target_os = "macos")]
    {
        return macos_system_memory_bytes();
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        (0, 0)
    }
}

#[allow(dead_code)]
pub(crate) fn parse_proc_meminfo(content: &str) -> (u64, u64) {
    let mut total = 0u64;
    let mut available = 0u64;
    let mut free = 0u64;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                .saturating_mul(1024);
        } else if line.starts_with("MemAvailable:") {
            available = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                .saturating_mul(1024);
        } else if line.starts_with("MemFree:") {
            free = line
                .split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
                .saturating_mul(1024);
        }
    }
    if available == 0 && free > 0 {
        available = free;
    }
    (total, available.min(total))
}

#[cfg(target_os = "macos")]
fn macos_system_memory_bytes() -> (u64, u64) {
    let total = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let vm_stat = std::process::Command::new("vm_stat")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let available = parse_macos_vm_stat(&vm_stat, total);
    (total, available)
}

pub(crate) fn parse_macos_vm_stat(vm_stat: &str, total_bytes: u64) -> u64 {
    let mut page_size = 16384u64;
    let mut free_pages = 0u64;
    let mut inactive_pages = 0u64;
    let mut speculative_pages = 0u64;

    for line in vm_stat.lines() {
        if let Some(idx) = line.find("page size of ") {
            let after = &line[idx + "page size of ".len()..];
            if let Some(num_str) = after.split_whitespace().next() {
                if let Ok(ps) = num_str.parse::<u64>() {
                    page_size = ps;
                }
            }
        } else if line.starts_with("Pages free:") {
            free_pages = parse_vm_stat_value(line);
        } else if line.starts_with("Pages inactive:") {
            inactive_pages = parse_vm_stat_value(line);
        } else if line.starts_with("Pages speculative:") {
            speculative_pages = parse_vm_stat_value(line);
        }
    }

    let avail = (free_pages + inactive_pages + speculative_pages).saturating_mul(page_size);
    if avail > 0 {
        if total_bytes > 0 {
            avail.min(total_bytes)
        } else {
            avail
        }
    } else {
        total_bytes / 2
    }
}

fn parse_vm_stat_value(line: &str) -> u64 {
    line.split(':')
        .nth(1)
        .map(|s| s.trim().trim_end_matches('.'))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

// --- Helper functions ---

#[cfg(target_os = "linux")]
fn read_sysfs_hex(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().trim_start_matches("0x");
    u64::from_str_radix(trimmed, 16).ok()
}

#[cfg(target_os = "linux")]
fn read_sysfs_u64(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    content.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn read_sysfs_string(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Get GPU name from PCI slot via lspci
#[cfg(target_os = "linux")]
fn pci_device_name(slot: &str) -> Option<String> {
    let output = std::process::Command::new("lspci")
        .arg("-s")
        .arg(slot)
        .output()
        .ok()?;
    let s = String::from_utf8(output.stdout).ok()?;
    // lspci output: "slot Class: Vendor Name [details]"
    let after_colon = s.split(": ").nth(1)?;
    // Take everything after the device class
    let name = after_colon.split(": ").last()?.trim();
    // Strip revision info
    let name = name.split(" (rev").next().unwrap_or(name).trim();
    Some(name.to_string())
}

/// Total system memory in bytes (from /proc/meminfo)
#[cfg(target_os = "linux")]
fn system_memory_total() -> u64 {
    system_memory_bytes().0
}

/// Available system memory in bytes
#[cfg(target_os = "linux")]
fn system_memory_available() -> u64 {
    system_memory_bytes().1
}

/// Detect NVIDIA GPUs via NVML (richer data than sysfs)
#[cfg(feature = "cuda")]
fn detect_cuda_nvml() -> Result<Vec<GpuDevice>, Box<dyn std::error::Error>> {
    use nvml_wrapper::Nvml;

    let nvml = Nvml::init()?;
    let device_count = nvml.device_count()?;
    let mut devices = Vec::new();

    for i in 0..device_count {
        let device = nvml.device_by_index(i)?;
        let name = device.name()?;
        let memory_info = device.memory_info()?;

        devices.push(GpuDevice::Cuda {
            index: i as usize,
            name,
            vram_total: memory_info.total,
            vram_free: memory_info.free,
        });
    }

    Ok(devices)
}

/// Detect Metal GPU on macOS
#[cfg(target_os = "macos")]
fn detect_metal_device() -> Option<GpuDevice> {
    use std::process::Command;

    let output = Command::new("sysctl")
        .arg("-n")
        .arg("hw.memsize")
        .output()
        .ok()?;

    let mem_str = String::from_utf8(output.stdout).ok()?;
    let unified_memory: u64 = mem_str.trim().parse().ok()?;

    Some(GpuDevice::Metal { unified_memory })
}

/// Get a summary of available compute devices
pub fn device_summary() -> String {
    let devices = detect_devices();
    let gpu_count = devices.iter().filter(|d| d.is_gpu()).count();

    if gpu_count == 0 {
        "CPU only (no GPU detected)".to_string()
    } else {
        let names: Vec<String> = devices
            .iter()
            .filter(|d| d.is_gpu())
            .map(|d| d.device_name())
            .collect();
        format!("{} GPU(s): {}", gpu_count, names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_proc_meminfo() {
        let sample = "MemTotal:       32768000 kB\nMemFree:         4000000 kB\nMemAvailable:   24576000 kB\n";
        let (total, avail) = parse_proc_meminfo(sample);
        assert_eq!(total, 32768000 * 1024);
        assert_eq!(avail, 24576000 * 1024);
    }

    #[test]
    fn parses_windows_memory_and_gpus() {
        let mem_json = r#"{"TotalVisibleMemorySize":67108864,"FreePhysicalMemory":50331648}"#;
        let (total, free) = parse_windows_memory_json(mem_json);
        assert_eq!(total, 67108864 * 1024);
        assert_eq!(free, 50331648 * 1024);

        let gpu_json = r#"[
            {"Name":"Microsoft Basic Display Adapter","AdapterRAM":0},
            {"Name":"NVIDIA GeForce RTX 4090","AdapterRAM":25769803776},
            {"Name":"AMD Radeon(TM) 8060S Graphics","AdapterRAM":4294967296}
        ]"#;
        let gpus = build_windows_gpu_devices(gpu_json, total, free, None);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vendor(), "NVIDIA");
        assert!(gpus[0].device_name().contains("RTX 4090"));
        assert_eq!(gpus[0].vram_total(), 25769803776);
        assert_eq!(gpus[1].vendor(), "AMD");
        assert!(gpus[1].device_name().contains("8060S"));
        assert!(gpus[1].vram_total() > 0);
    }

    #[test]
    fn parses_macos_vm_stat() {
        let vm = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free:                              100000.\nPages active:                            200000.\nPages inactive:                          150000.\nPages speculative:                        50000.\n";
        let total = 16 * 1024 * 1024 * 1024u64;
        let avail = parse_macos_vm_stat(vm, total);
        assert_eq!(avail, (100000 + 150000 + 50000) * 16384);
    }

    #[test]
    fn cpu_fallback_always_present() {
        let devices = detect_devices();
        assert!(devices.iter().any(|d| matches!(d, GpuDevice::Cpu)));
    }
}
