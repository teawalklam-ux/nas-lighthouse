#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn attach_parent_console() {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(dw_process_id: u32) -> i32;
            fn AllocConsole() -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                AllocConsole();
            }
        }
    }
}

fn main() {
    let probe = std::env::args().any(|a| a == "--probe");
    if probe {
        attach_parent_console();
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        match rt.block_on(nas_lighthouse_lib::run_probe()) {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
    }
    nas_lighthouse_lib::run();
}
