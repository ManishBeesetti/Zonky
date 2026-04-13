//! Auto-detection and installation of GPU compute dependencies.
//!
//! Scans the system for GPU hardware, checks what software is installed,
//! and provides functions to install missing packages.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use super::detect_devices;

/// A required package that may or may not be installed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub description: String,
    pub installed: bool,
    pub install_cmd: Option<String>,
    pub required_by: String,
}

/// Overall setup status for GPU compute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub gpu_vendor: String,
    pub gpu_name: String,
    pub compute_ready: bool,
    pub needs_reboot: bool,
    pub deps: Vec<Dependency>,
    pub summary: String,
}

/// Detect GPU hardware and check all necessary dependencies.
pub fn check_setup() -> SetupStatus {
    let devices = detect_devices();

    // Find the best GPU (skip CPU)
    let gpu = devices.iter().find(|d| d.is_gpu());

    match gpu {
        Some(super::GpuDevice::Rocm { name, .. }) => check_amd_setup(name),
        Some(super::GpuDevice::Cuda { name, .. }) => check_nvidia_setup(name),
        Some(super::GpuDevice::Metal { .. }) => check_metal_setup(),
        _ => SetupStatus {
            gpu_vendor: "None".into(),
            gpu_name: "No GPU detected".into(),
            compute_ready: false,
            needs_reboot: false,
            deps: vec![],
            summary: "No GPU hardware found. CPU inference will be used.".into(),
        },
    }
}

/// Generate a single install command for all missing dependencies.
pub fn install_command(status: &SetupStatus) -> Option<String> {
    let missing: Vec<&str> = status
        .deps
        .iter()
        .filter(|d| !d.installed && d.install_cmd.is_some())
        .filter_map(|d| d.install_cmd.as_deref())
        .collect();

    if missing.is_empty() {
        return None;
    }

    // Deduplicate apt packages into a single command
    let mut apt_packages = Vec::new();
    let mut other_cmds = Vec::new();

    for cmd in &missing {
        if let Some(pkg) = cmd.strip_prefix("apt:") {
            apt_packages.push(pkg);
        } else {
            other_cmds.push(*cmd);
        }
    }

    let mut commands = Vec::new();
    if !apt_packages.is_empty() {
        commands.push(format!("sudo apt install -y {}", apt_packages.join(" ")));
    }
    commands.extend(other_cmds.iter().map(|c| c.to_string()));

    Some(commands.join(" && "))
}

/// Run the installation of all missing dependencies.
/// Returns Ok(output) on success, Err(message) on failure.
pub fn run_install(status: &SetupStatus) -> Result<String, String> {
    let cmd = install_command(status).ok_or("Nothing to install — all dependencies satisfied.")?;

    info!("Running: {}", cmd);

    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("Failed to execute install command: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(format!("{stdout}\n{stderr}"))
    } else {
        Err(format!(
            "Install failed (exit code {}):\n{stderr}",
            output.status.code().unwrap_or(-1)
        ))
    }
}

// --- AMD ROCm checks ---

fn check_amd_setup(gpu_name: &str) -> SetupStatus {
    let distro = detect_distro();
    let mut deps = Vec::new();

    // 1. Kernel compute driver (/dev/kfd)
    deps.push(Dependency {
        name: "amdgpu kernel driver".into(),
        description: "Kernel driver for AMD GPU compute (/dev/kfd)".into(),
        installed: Path::new("/dev/kfd").exists(),
        install_cmd: if !Path::new("/dev/kfd").exists() {
            Some("apt:linux-modules-extra-$(uname -r)".into())
        } else {
            None
        },
        required_by: "ROCm compute".into(),
    });

    // 2. ROCm runtime
    let rocm_installed = Path::new("/opt/rocm").exists() || which_exists("rocminfo");
    deps.push(Dependency {
        name: "rocm-dev".into(),
        description: "AMD ROCm development runtime".into(),
        installed: rocm_installed,
        install_cmd: if !rocm_installed {
            Some(rocm_install_cmd(&distro))
        } else {
            None
        },
        required_by: "ROCm compute".into(),
    });

    // 3. rocminfo (diagnostic tool)
    let has_rocminfo = which_exists("rocminfo");
    deps.push(Dependency {
        name: "rocminfo".into(),
        description: "ROCm system info utility".into(),
        installed: has_rocminfo,
        install_cmd: if !has_rocminfo { Some("apt:rocminfo".into()) } else { None },
        required_by: "GPU diagnostics".into(),
    });

    // 4. HIP runtime (for hipBLAS / candle)
    let has_hip = Path::new("/opt/rocm/lib/libamdhip64.so").exists()
        || Path::new("/usr/lib/libamdhip64.so").exists()
        || which_exists("hipcc");
    deps.push(Dependency {
        name: "hip-runtime-amd".into(),
        description: "AMD HIP runtime for GPU compute kernels".into(),
        installed: has_hip,
        install_cmd: if !has_hip { Some("apt:hip-runtime-amd".into()) } else { None },
        required_by: "Candle ROCm backend".into(),
    });

    // 5. hipBLAS (BLAS on AMD GPU)
    let has_hipblas = Path::new("/opt/rocm/lib/libhipblas.so").exists()
        || pkg_config_exists("hipblas");
    deps.push(Dependency {
        name: "hipblas-dev".into(),
        description: "HIP BLAS library for matrix operations".into(),
        installed: has_hipblas,
        install_cmd: if !has_hipblas { Some("apt:hipblas-dev".into()) } else { None },
        required_by: "Candle ROCm backend".into(),
    });

    // 6. User in 'render' and 'video' groups
    let in_render = user_in_group("render");
    let in_video = user_in_group("video");
    deps.push(Dependency {
        name: "GPU group membership".into(),
        description: "User must be in 'render' and 'video' groups".into(),
        installed: in_render && in_video,
        install_cmd: if !(in_render && in_video) {
            let user = std::env::var("USER").unwrap_or_else(|_| "$(whoami)".into());
            let mut cmds = Vec::new();
            if !in_render { cmds.push(format!("sudo usermod -aG render {user}")); }
            if !in_video { cmds.push(format!("sudo usermod -aG video {user}")); }
            Some(cmds.join(" && "))
        } else {
            None
        },
        required_by: "GPU access permissions".into(),
    });

    // 7. Build tools
    check_build_tools(&mut deps);

    let all_good = deps.iter().all(|d| d.installed);
    let reboot = needs_reboot_for_groups();

    SetupStatus {
        gpu_vendor: "AMD".into(),
        gpu_name: gpu_name.to_string(),
        compute_ready: all_good && !reboot,
        needs_reboot: reboot,
        summary: if all_good && !reboot {
            "All AMD ROCm dependencies are installed. GPU compute is ready.".into()
        } else if reboot {
            "Group membership changed — reboot required to activate GPU access.".into()
        } else {
            let missing_count = deps.iter().filter(|d| !d.installed).count();
            format!("{missing_count} missing dependency(ies). Run `zonky setup --install` to install them.")
        },
        deps,
    }
}

fn check_nvidia_setup(gpu_name: &str) -> SetupStatus {
    let mut deps = Vec::new();

    // 1. NVIDIA driver
    let has_driver = Path::new("/dev/nvidia0").exists() || which_exists("nvidia-smi");
    deps.push(Dependency {
        name: "nvidia-driver".into(),
        description: "NVIDIA GPU driver".into(),
        installed: has_driver,
        install_cmd: if !has_driver { Some("apt:nvidia-driver-580".into()) } else { None },
        required_by: "CUDA compute".into(),
    });

    // 2. CUDA toolkit
    let has_cuda = which_exists("nvcc") || Path::new("/usr/local/cuda").exists();
    deps.push(Dependency {
        name: "cuda-toolkit".into(),
        description: "NVIDIA CUDA toolkit".into(),
        installed: has_cuda,
        install_cmd: if !has_cuda { Some("apt:nvidia-cuda-toolkit".into()) } else { None },
        required_by: "Candle CUDA backend".into(),
    });

    // 3. cuBLAS
    let has_cublas = Path::new("/usr/local/cuda/lib64/libcublas.so").exists()
        || Path::new("/usr/lib/x86_64-linux-gnu/libcublas.so").exists()
        || pkg_config_exists("cublas");
    deps.push(Dependency {
        name: "libcublas-dev".into(),
        description: "CUDA BLAS library for matrix operations".into(),
        installed: has_cublas,
        install_cmd: if !has_cublas { Some("apt:libcublas-dev".into()) } else { None },
        required_by: "Candle CUDA backend".into(),
    });

    // 4. Build tools
    check_build_tools(&mut deps);

    let all_good = deps.iter().all(|d| d.installed);

    SetupStatus {
        gpu_vendor: "NVIDIA".into(),
        gpu_name: gpu_name.to_string(),
        compute_ready: all_good,
        needs_reboot: false,
        summary: if all_good {
            "All NVIDIA CUDA dependencies are installed. GPU compute is ready.".into()
        } else {
            let missing_count = deps.iter().filter(|d| !d.installed).count();
            format!("{missing_count} missing dependency(ies). Run `zonky setup --install` to install them.")
        },
        deps,
    }
}

fn check_metal_setup() -> SetupStatus {
    // Metal on macOS needs virtually nothing extra
    SetupStatus {
        gpu_vendor: "Apple".into(),
        gpu_name: "Apple Silicon (Metal)".into(),
        compute_ready: true,
        needs_reboot: false,
        deps: vec![],
        summary: "Metal is natively supported. No additional dependencies needed.".into(),
    }
}

// --- Shared dependency checks ---

fn check_build_tools(deps: &mut Vec<Dependency>) {
    let has_gcc = which_exists("gcc") || which_exists("cc");
    deps.push(Dependency {
        name: "gcc".into(),
        description: "C compiler (required to build native code)".into(),
        installed: has_gcc,
        install_cmd: if !has_gcc { Some("apt:build-essential".into()) } else { None },
        required_by: "Rust native compilation".into(),
    });

    let has_cmake = which_exists("cmake");
    deps.push(Dependency {
        name: "cmake".into(),
        description: "Build system generator".into(),
        installed: has_cmake,
        install_cmd: if !has_cmake { Some("apt:cmake".into()) } else { None },
        required_by: "Candle native builds".into(),
    });

    let has_pkg_config = which_exists("pkg-config");
    deps.push(Dependency {
        name: "pkg-config".into(),
        description: "Package configuration tool".into(),
        installed: has_pkg_config,
        install_cmd: if !has_pkg_config { Some("apt:pkg-config".into()) } else { None },
        required_by: "Rust native compilation".into(),
    });
}

// --- Utility functions ---

fn which_exists(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pkg_config_exists(lib: &str) -> bool {
    Command::new("pkg-config")
        .arg("--exists")
        .arg(lib)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn user_in_group(group: &str) -> bool {
    // First check the current session's groups (reflects active state)
    let session_has = Command::new("id")
        .arg("-nG")
        .output()
        .map(|o| {
            let groups = String::from_utf8_lossy(&o.stdout);
            groups.split_whitespace().any(|g| g == group)
        })
        .unwrap_or(false);

    if session_has {
        return true;
    }

    // Fall back to /etc/group (reflects persistent state even before reboot)
    user_in_group_etc(group)
}

fn user_in_group_etc(group: &str) -> bool {
    let user = std::env::var("USER").unwrap_or_default();
    if user.is_empty() {
        return false;
    }
    std::fs::read_to_string("/etc/group")
        .ok()
        .map(|content| {
            content.lines().any(|line| {
                let parts: Vec<&str> = line.split(':').collect();
                parts.first() == Some(&group)
                    && parts.get(3).map_or(false, |members| {
                        members.split(',').any(|m| m == user)
                    })
            })
        })
        .unwrap_or(false)
}

/// Check if group membership was added but requires a reboot/relogin to take effect.
pub fn needs_reboot_for_groups() -> bool {
    let groups = ["render", "video"];
    groups.iter().any(|g| {
        // In /etc/group but NOT in current session
        user_in_group_etc(g) && !Command::new("id")
            .arg("-nG")
            .output()
            .map(|o| {
                let out = String::from_utf8_lossy(&o.stdout);
                out.split_whitespace().any(|s| s == *g)
            })
            .unwrap_or(false)
    })
}

fn detect_distro() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("ID="))
                .map(|l| l.trim_start_matches("ID=").trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn rocm_install_cmd(distro: &str) -> String {
    match distro {
        "ubuntu" | "debian" | "linuxmint" | "pop" => {
            // Modern Ubuntu has ROCm in the default repos
            "apt:rocm-dev".into()
        }
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            debug!("Fedora/RHEL ROCm install requires AMD repo setup");
            "apt:rocm-dev".into() // Simplified; real install needs repo setup
        }
        _ => {
            debug!("Unknown distro {distro} — falling back to apt");
            "apt:rocm-dev".into()
        }
    }
}
