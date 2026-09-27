use std::ffi::CString;

const INFO: i32 = 4;
const WARN: i32 = 5;

unsafe extern "C" {
    fn __android_log_write(
        prio: i32,
        tag: *const core::ffi::c_char,
        text: *const core::ffi::c_char,
    ) -> i32;
    fn pipe(fds: *mut i32) -> i32;
    fn dup2(old: i32, new: i32) -> i32;
}

pub(crate) fn write(message: &str) {
    log(INFO, message);
}

fn log(priority: i32, message: &str) {
    let text = match CString::new(message) {
        Ok(text) => text,
        Err(e) => {
            let up_to_nul = e.into_vec();
            let cut = up_to_nul.iter().position(|b| *b == 0).unwrap_or(0);
            match CString::new(&up_to_nul[..cut]) {
                Ok(text) => text,
                Err(_) => return,
            }
        }
    };
    let Ok(tag) = CString::new("torch-client") else {
        return;
    };
    unsafe { __android_log_write(priority, tag.as_ptr(), text.as_ptr()) };
}

pub(crate) fn redirect_stdio() {
    use std::io::BufRead;
    use std::sync::atomic::{AtomicBool, Ordering};

    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut fds = [0i32; 2];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        log(WARN, "stdout could not be redirected to logcat");
        return;
    }
    let [read_fd, write_fd] = fds;
    unsafe {
        dup2(write_fd, 1);
        dup2(write_fd, 2);
    }

    std::thread::Builder::new()
        .name("logcat".to_owned())
        .spawn(move || {
            let pipe = unsafe { <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(read_fd) };
            let mut reader = std::io::BufReader::new(pipe);
            let mut line = Vec::new();
            while let Ok(read) = reader.read_until(b'\n', &mut line) {
                if read == 0 {
                    break;
                }
                let text = String::from_utf8_lossy(&line);
                write(text.trim_end());
                line.clear();
            }
        })
        .ok();
}
