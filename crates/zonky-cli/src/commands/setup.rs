use console::style;
use dialoguer::Confirm;

use zonky_core::gpu;

pub fn run(install: bool) {
    let status = gpu::setup::check_setup();

    println!();
    println!(
        "{} Zonky GPU Setup",
        style("[SETUP]").bold()
    );
    println!(
        "   GPU: {} ({})",
        style(&status.gpu_name).bold().cyan(),
        style(&status.gpu_vendor).dim()
    );
    println!();

    if status.deps.is_empty() {
        println!(
            "  {} {}",
            style("[OK]").green().bold(),
            status.summary
        );
        println!();
        return;
    }

    // Print dependency table
    println!("  {}", style("Dependencies:").bold().underlined());
    println!();

    for dep in &status.deps {
        let icon = if dep.installed {
            style("[OK]").green().bold()
        } else {
            style("[ERR]").red().bold()
        };

        let status_text = if dep.installed {
            style("installed").green()
        } else {
            style("missing").red()
        };

        println!(
            "  {icon} {:<28} [{status_text}]   {}",
            style(&dep.name).bold(),
            style(&dep.description).dim()
        );
    }

    println!();

    // Check if reboot is needed (groups changed but not active in session)
    if status.needs_reboot {
        println!(
            "  {} {}",
            style("[WARN]").yellow().bold(),
            style("Group membership was updated but requires a reboot to take effect.").yellow()
        );
        println!();

        if Confirm::new()
            .with_prompt("  Would you like to reboot now?")
            .default(false)
            .interact()
            .unwrap_or(false)
        {
            println!("\n  {} Rebooting...\n", style("[->]").cyan().bold());
            let _ = std::process::Command::new("sudo")
                .arg("reboot")
                .status();
        } else {
            println!("\n  Please reboot or log out and back in when convenient.");
            println!("  Then run {} to verify.\n", style("zonky setup").cyan());
        }
        return;
    }

    if status.compute_ready {
        println!(
            "  {} {}",
            style("[OK]").green().bold(),
            style("All dependencies satisfied. GPU compute is ready!").green().bold()
        );
        println!();
        return;
    }

    // Show what's missing
    let missing: Vec<&gpu::setup::Dependency> = status.deps.iter().filter(|d| !d.installed).collect();
    println!(
        "  {} {} missing:",
        style("[WARN]").yellow().bold(),
        missing.len()
    );
    for dep in &missing {
        println!("     {}: {}", style(&dep.name).bold(), dep.description);
    }
    println!();

    if let Some(cmd) = gpu::setup::install_command(&status) {
        if install {
            println!(
                "  {} Installing dependencies...\n",
                style("[->]").cyan().bold()
            );
            println!("  {}\n", style(&cmd).dim());

            match gpu::setup::run_install(&status) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        let lines: Vec<&str> = output.lines().collect();
                        let start = lines.len().saturating_sub(5);
                        for line in &lines[start..] {
                            println!("    {}", style(line).dim());
                        }
                        println!();
                    }
                    println!(
                        "  {} Installation complete!",
                        style("[OK]").green().bold()
                    );

                    // Re-check
                    let recheck = gpu::setup::check_setup();
                    if recheck.compute_ready {
                        println!(
                            "  {} GPU compute is ready.\n",
                            style("[OK]").green().bold()
                        );
                    } else if recheck.needs_reboot {
                        println!(
                            "\n  {} {}",
                            style("[WARN]").yellow().bold(),
                            style("Group membership updated - reboot required to activate GPU access.").yellow()
                        );
                        println!();

                        if Confirm::new()
                            .with_prompt("  Would you like to reboot now?")
                            .default(false)
                            .interact()
                            .unwrap_or(false)
                        {
                            println!("\n  {} Rebooting...\n", style("[->]").cyan().bold());
                            let _ = std::process::Command::new("sudo")
                                .arg("reboot")
                                .status();
                        } else {
                            println!("\n  Please reboot or log out and back in when convenient.");
                            println!("  Then run {} to verify.\n", style("zonky setup").cyan());
                        }
                    } else {
                        let still_missing: Vec<_> = recheck.deps.iter()
                            .filter(|d| !d.installed)
                            .collect();
                        if !still_missing.is_empty() {
                            println!(
                                "\n  {} Some dependencies still need attention:",
                                style("[WARN]").yellow().bold()
                            );
                            for dep in &still_missing {
                                println!("     {}", dep.name);
                            }
                            println!();
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "  {} Installation failed:\n    {}",
                        style("[ERR]").red().bold(),
                        style(e).red()
                    );
                    println!("\n  Try running manually:\n    {}", style(&cmd).bold());
                }
            }
        } else {
            println!("  To install all missing dependencies, run:\n");
            println!("    {}", style("zonky setup --install").bold().cyan());
            println!("\n  Or manually:\n");
            println!("    {}", style(&cmd).dim());
        }
    }

    println!();
}
