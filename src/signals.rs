use std::sync::atomic::{AtomicBool, Ordering};

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn install() {
    STOP_REQUESTED.store(false, Ordering::SeqCst);
    unsafe {
        install_platform_handler();
    }
}

pub fn stop_requested() -> bool {
    STOP_REQUESTED.load(Ordering::SeqCst)
}

pub fn request_stop() {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
}

#[cfg(unix)]
unsafe fn install_platform_handler() {
    extern "C" fn handle_signal(_: i32) {
        STOP_REQUESTED.store(true, Ordering::SeqCst);
    }

    unsafe extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> extern "C" fn(i32);
    }

    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;

    let _ = unsafe { signal(SIGINT, handle_signal) };
    let _ = unsafe { signal(SIGTERM, handle_signal) };
}

#[cfg(windows)]
unsafe fn install_platform_handler() {
    type HandlerRoutine = Option<unsafe extern "system" fn(u32) -> i32>;

    unsafe extern "system" fn handle_console(ctrl_type: u32) -> i32 {
        const CTRL_C_EVENT: u32 = 0;
        const CTRL_BREAK_EVENT: u32 = 1;
        if ctrl_type == CTRL_C_EVENT || ctrl_type == CTRL_BREAK_EVENT {
            STOP_REQUESTED.store(true, Ordering::SeqCst);
            1
        } else {
            0
        }
    }

    extern "system" {
        fn SetConsoleCtrlHandler(handler: HandlerRoutine, add: i32) -> i32;
    }

    let _ = unsafe { SetConsoleCtrlHandler(Some(handle_console), 1) };
}

#[cfg(not(any(unix, windows)))]
unsafe fn install_platform_handler() {}
