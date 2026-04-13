pub mod setup;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
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
            GpuDevice::Rocm { name, index, is_apu, .. } => {
                let suffix = if *is_apu { " [APU]" } else { "" };
                format!("ROCm:{index} ({name}{suffix})")
            }
            GpuDevice::Metal { unified_memory } => {
                format!("Metal ({}GB unified)", unified_memory / (1024 * 1024 * 1024))
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

    // CUDA via NVML (if compiled with cuda feature, supplements sysfs data)
    #[cfg(feature = "cuda")]
    {
        if let Ok(cuda_devices) = detect_cuda_nvml() {
            // Replace any sysfs NVIDIA entries with richer NVML data
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
                if let Some(dev) = detect_nvidia_sysfs(&device_path, devices.iter().filter(|d| matches!(d, GpuDevice::Cuda { .. })).count()) {
                    devices.push(dev);
                }
            }
            0x1002 => {
                // AMD
                if let Some(dev) = detect_amd_sysfs(&device_path, devices.iter().filter(|d| matches!(d, GpuDevice::Rocm { .. })).count()) {
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
            let pci_slot = read_sysfs_string(&device_path.join("uevent"))
                .and_then(|s| {
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
            let pci_slot = read_sysfs_string(&device_path.join("uevent"))
                .and_then(|s| {
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
                        l.split_whitespace().nth(1).and_then(|v| v.parse::<u64>().ok())
                    })
            })
            .unwrap_or(0) * 1024; // meminfo is in kB
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

fn read_sysfs_string(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
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
fn system_memory_total() -> u64 {
    read_sysfs_string(Path::new("/proc/meminfo"))
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemTotal:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .unwrap_or(0) * 1024
}

/// Available system memory in bytes
fn system_memory_available() -> u64 {
    read_sysfs_string(Path::new("/proc/meminfo"))
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemAvailable:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .unwrap_or(0) * 1024
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
