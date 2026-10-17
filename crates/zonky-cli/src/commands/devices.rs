use console::style;

use zonky_core::gpu;

pub fn run() {
    println!("\n{} System Devices:\n", style("[HW]").bold());

    let devices = gpu::detect_devices();

    for (i, device) in devices.iter().enumerate() {
        match device {
            gpu::GpuDevice::Cuda {
                index,
                name,
                vram_total,
                vram_free,
            } => {
                println!(
                    "  {} CUDA:{} - {}",
                    style(format!("[{i}]")).dim(),
                    index,
                    style(name).bold()
                );
                if *vram_total > 0 {
                    println!(
                        "      VRAM: {} / {} ({} free)",
                        bytesize::ByteSize(*vram_free),
                        bytesize::ByteSize(*vram_total),
                        style(format!(
                            "{}%",
                            (*vram_free as f64 / *vram_total as f64 * 100.0) as u32
                        ))
                        .green()
                    );
                }
            }
            gpu::GpuDevice::Rocm {
                index,
                name,
                vram_total,
                vram_free,
                is_apu,
            } => {
                let apu_tag = if *is_apu { " [APU - shared memory]" } else { "" };
                println!(
                    "  {} ROCm:{} - {}{}",
                    style(format!("[{i}]")).dim(),
                    index,
                    style(name).bold(),
                    style(apu_tag).dim()
                );
                if *vram_total > 0 {
                    let label = if *is_apu { "Memory pool" } else { "VRAM" };
                    println!(
                        "      {}: {} / {} ({} free)",
                        label,
                        bytesize::ByteSize(*vram_free),
                        bytesize::ByteSize(*vram_total),
                        style(format!(
                            "{}%",
                            (*vram_free as f64 / *vram_total as f64 * 100.0) as u32
                        ))
                        .green()
                    );
                }
                // Check compute readiness
                let has_kfd = std::path::Path::new("/dev/kfd").exists();
                if has_kfd {
                    println!("      Compute: {} (kernel driver active)", style("ready").green());
                } else {
                    println!("      Compute: {} (run `zonky setup` to fix)", style("not available").red());
                }
            }
            gpu::GpuDevice::Metal { unified_memory } => {
                println!(
                    "  {} Metal - Apple Silicon",
                    style(format!("[{i}]")).dim(),
                );
                println!(
                    "      Unified Memory: {}",
                    bytesize::ByteSize(*unified_memory)
                );
            }
            gpu::GpuDevice::Cpu => {
                println!("  {} CPU (fallback)", style(format!("[{i}]")).dim());
            }
        }
        println!();
    }

    println!("  {}", style(gpu::device_summary()).bold());
    println!();
}
