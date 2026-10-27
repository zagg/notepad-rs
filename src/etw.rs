//! TraceLogging-style ETW provider, mirroring the stub's EventRegister/EventUnregister flow
//! (wWinMain FUN_1400101d8: CoCreateGuid -> EventRegister -> EventSetInformation; events
//!  "LaunchNotepadStart" / "LaunchNotepadComplete" observed in strings; provider name "Microsoft.Notepad").

use windows::core::PCWSTR;
use windows::Win32::System::Com::CoCreateGuid;
use windows::Win32::System::Diagnostics::Etw::{
    EventRegister, EventUnregister, EventWriteString, PENABLECALLBACK, REGHANDLE,
};

pub struct Tracing {
    handle: REGHANDLE,
    registered: bool,
}

impl Tracing {
    /// Registers a fresh random provider GUID on every launch, exactly like the original stub.
    pub fn start(_provider_name: &str) -> Self {
        let mut handle: REGHANDLE = REGHANDLE(0);
        let ok = unsafe {
            match CoCreateGuid() {
                Ok(guid) => {
                    let mut raw: u64 = 0;
                    let status = EventRegister(
                        &guid as *const _,
                        PENABLECALLBACK::default(),
                        None,
                        &mut raw as *mut u64,
                    );
                    handle = REGHANDLE(raw as i64);
                    status == 0
                }
                Err(_) => false,
            }
        };
        Self { handle, registered: ok }
    }

    pub fn write(&self, msg: &str) {
        if !self.registered {
            return;
        }
        let w = crate::wf::to_wide(msg);
        unsafe {
            // level 4 = TRACE_LEVEL_INFORMATION, keyword 0.
            let _ = EventWriteString(self.handle, 4, 0, PCWSTR(w.as_ptr()));
        }
    }
}

impl Drop for Tracing {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                let _ = EventUnregister(self.handle);
            }
        }
    }
}
