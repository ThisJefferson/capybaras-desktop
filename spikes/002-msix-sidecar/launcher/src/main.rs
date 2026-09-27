// SPIKE CODE - throwaway. No dependencies.
//
// Stands in for the Tauri shell: a full-trust MSIX app entry point that spawns a
// Node sidecar bundled INSIDE the package. The question under test is whether
// app-local spawn works from WindowsApps (a read-only, virtualised location).

use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();

    let out_dir = format!(
        "{}\\CapybarasSpike",
        std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into())
    );
    let _ = fs::create_dir_all(&out_dir);
    let log_path = format!("{}\\launcher.log", out_dir);

    let node = dir.join("node.exe");
    let script = dir.join("sidecar.mjs");

    let mut r = String::new();
    r.push_str("== full-trust MSIX launcher ==\n");
    r.push_str(&format!("launcher exe     : {}\n", exe.display()));
    r.push_str(&format!("app dir          : {}\n", dir.display()));
    r.push_str(&format!("node.exe present : {}\n", node.is_file()));
    r.push_str(&format!("sidecar present  : {}\n", script.is_file()));
    r.push_str(&format!("app dir writable : {}\n", probe_write(&dir)));
    r.push_str(&format!("out dir writable : {}\n", probe_write(Path::new(&out_dir))));

    // The load-bearing step: spawn the bundled Node from app-local storage.
    match Command::new(&node)
        .arg(&script)
        .arg(format!("--proof={}\\sidecar-proof.json", out_dir))
        .output()
    {
        Ok(o) => {
            r.push_str(&format!("node spawn       : OK (exit {})\n", o.status));
            r.push_str("--- node stdout ---\n");
            r.push_str(&String::from_utf8_lossy(&o.stdout));
            r.push_str("--- node stderr ---\n");
            r.push_str(&String::from_utf8_lossy(&o.stderr));
        }
        Err(e) => {
            r.push_str(&format!("node spawn       : FAILED: {}\n", e));
        }
    }

    let _ = fs::write(&log_path, r);
}

fn probe_write(dir: &Path) -> String {
    let p = dir.join(".write-probe");
    match fs::write(&p, b"x") {
        Ok(_) => {
            let _ = fs::remove_file(&p);
            "yes".to_string()
        }
        Err(e) => format!("NO ({:?})", e.kind()),
    }
}
