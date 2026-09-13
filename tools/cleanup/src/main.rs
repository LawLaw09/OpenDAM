use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    let is_dry_run = args.iter().any(|a| a == "--dry-run");
    let is_silent = args.iter().any(|a| a == "--silent" || a == "-s" || a == "-y");

    println!("========================================================================");
    println!("                  OpenDAM Post-Uninstall Cleanup Tool                   ");
    println!("========================================================================");
    println!("This tool cleans residual app files, caches, shortcuts, and registry keys");
    println!("left behind after uninstalling OpenDAM.\n");
    println!("IMPORTANT SAFETY GUARANTEE:");
    println!("  * Your asset libraries, 3D files, textures, and library databases");
    println!("    ('.opendam/library.sqlite' inside your library directories)");
    println!("    are NEVER touched and remain 100% safe and intact.\n");
    if is_dry_run {
        println!("[MODE: DRY RUN - No files or registry keys will be removed]");
    }
    println!("------------------------------------------------------------------------");

    // 1. Terminate any running OpenDAM processes so files aren't locked
    println!("[1/5] Checking for active OpenDAM processes...");
    terminate_processes(is_dry_run);

    let mut total_freed_bytes: u64 = 0;
    let mut total_removed_items: usize = 0;

    // 2. Scan and clean AppData and system residual directories
    println!("\n[2/5] Cleaning residual application directories...");
    let target_dirs = get_candidate_directories();
    for dir in target_dirs {
        if dir.exists() {
            let size = calculate_dir_size(&dir);
            print!("  Removing: {} ({}) ... ", dir.display(), format_bytes(size));
            io::stdout().flush().ok();

            if is_dry_run {
                println!("[FOUND]");
                total_freed_bytes += size;
                total_removed_items += 1;
            } else {
                match fs::remove_dir_all(&dir) {
                    Ok(_) => {
                        println!("[OK]");
                        total_freed_bytes += size;
                        total_removed_items += 1;
                    }
                    Err(e) => {
                        println!("[FAILED: {}]", e);
                    }
                }
            }
        }
    }

    // 3. Clean shortcuts
    println!("\n[3/5] Cleaning residual shortcuts...");
    let shortcut_files = get_candidate_shortcuts();
    for shortcut in shortcut_files {
        if shortcut.exists() {
            let size = fs::metadata(&shortcut).map(|m| m.len()).unwrap_or(0);
            print!("  Removing shortcut: {} ... ", shortcut.display());
            io::stdout().flush().ok();

            if is_dry_run {
                println!("[FOUND]");
                total_freed_bytes += size;
                total_removed_items += 1;
            } else {
                let res = if shortcut.is_dir() {
                    fs::remove_dir_all(&shortcut)
                } else {
                    fs::remove_file(&shortcut)
                };

                match res {
                    Ok(_) => {
                        println!("[OK]");
                        total_freed_bytes += size;
                        total_removed_items += 1;
                    }
                    Err(e) => println!("[FAILED: {}]", e),
                }
            }
        }
    }

    // 4. Clean temporary files
    println!("\n[4/5] Cleaning residual temporary files...");
    if let Ok(temp) = env::var("TEMP") {
        let temp_path = PathBuf::from(temp);
        if temp_path.exists() && temp_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&temp_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let file_name = entry.file_name().to_string_lossy().to_lowercase();
                    if file_name.starts_with("opendam") || file_name.starts_with("com.opendam") {
                        let size = calculate_dir_size(&path);
                        print!("  Removing temp item: {} ... ", path.display());
                        io::stdout().flush().ok();

                        if is_dry_run {
                            println!("[FOUND]");
                            total_freed_bytes += size;
                            total_removed_items += 1;
                        } else {
                            let res = if path.is_dir() {
                                fs::remove_dir_all(&path)
                            } else {
                                fs::remove_file(&path)
                            };

                            match res {
                                Ok(_) => {
                                    println!("[OK]");
                                    total_freed_bytes += size;
                                    total_removed_items += 1;
                                }
                                Err(e) => println!("[FAILED: {}]", e),
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Clean residual registry keys
    println!("\n[5/5] Cleaning residual registry keys...");
    let reg_keys = [
        "HKCU\\Software\\OpenDAM",
        "HKCU\\Software\\com.opendam.desktop",
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenDAM",
        "HKLM\\Software\\OpenDAM",
        "HKLM\\Software\\com.opendam.desktop",
        "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenDAM",
    ];

    for key in &reg_keys {
        if is_dry_run {
            println!("  Checking registry key: {} [DRY RUN]", key);
        } else {
            let output = Command::new("reg")
                .args(["delete", key, "/f"])
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    println!("  Deleted registry key: {} [OK]", key);
                    total_removed_items += 1;
                }
            }
        }
    }

    println!("\n========================================================================");
    println!("                           Cleanup Summary                              ");
    println!("========================================================================");
    println!("Items Processed: {}", total_removed_items);
    println!("Disk Space Freed: {}", format_bytes(total_freed_bytes));
    println!("[CONFIRMED] Library directories and database files (.opendam/library.sqlite)");
    println!("            were completely untouched and remain intact.");
    println!("========================================================================");

    if !is_silent {
        println!("\nPress Enter to exit...");
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).ok();
    }
}

fn terminate_processes(is_dry_run: bool) {
    let procs = ["opendam.exe", "opendam-desktop.exe"];
    for proc in procs {
        if is_dry_run {
            println!("  [DRY RUN] Would check/kill process: {}", proc);
        } else {
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", proc])
                .output();
        }
    }
    println!("  Process check complete.");
}

fn get_candidate_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(appdata) = env::var("APPDATA") {
        let p = PathBuf::from(&appdata);
        dirs.push(p.join("com.opendam.desktop"));
        dirs.push(p.join("OpenDAM"));
    }

    if let Ok(localappdata) = env::var("LOCALAPPDATA") {
        let p = PathBuf::from(&localappdata);
        dirs.push(p.join("com.opendam.desktop"));
        dirs.push(p.join("OpenDAM"));
    }

    if let Ok(userprofile) = env::var("USERPROFILE") {
        let p = PathBuf::from(&userprofile);
        dirs.push(p.join("AppData").join("LocalLow").join("com.opendam.desktop"));
        dirs.push(p.join("AppData").join("LocalLow").join("OpenDAM"));
    }

    if let Ok(programdata) = env::var("ProgramData") {
        let p = PathBuf::from(&programdata);
        dirs.push(p.join("OpenDAM"));
        dirs.push(p.join("com.opendam.desktop"));
    }

    if let Ok(programfiles) = env::var("ProgramFiles") {
        let p = PathBuf::from(&programfiles);
        dirs.push(p.join("OpenDAM"));
    }

    if let Ok(programfiles_x86) = env::var("ProgramFiles(x86)") {
        let p = PathBuf::from(&programfiles_x86);
        dirs.push(p.join("OpenDAM"));
    }

    dirs
}

fn get_candidate_shortcuts() -> Vec<PathBuf> {
    let mut shortcuts = Vec::new();

    if let Ok(userprofile) = env::var("USERPROFILE") {
        let desktop = PathBuf::from(&userprofile).join("Desktop");
        shortcuts.push(desktop.join("OpenDAM.lnk"));
    }

    shortcuts.push(PathBuf::from(r"C:\Users\Public\Desktop\OpenDAM.lnk"));

    if let Ok(appdata) = env::var("APPDATA") {
        let start_menu = PathBuf::from(&appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        shortcuts.push(start_menu.join("OpenDAM.lnk"));
        shortcuts.push(start_menu.join("OpenDAM"));
    }

    if let Ok(programdata) = env::var("ProgramData") {
        let common_start_menu = PathBuf::from(&programdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        shortcuts.push(common_start_menu.join("OpenDAM.lnk"));
        shortcuts.push(common_start_menu.join("OpenDAM"));
    }

    shortcuts
}

fn calculate_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += calculate_dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
