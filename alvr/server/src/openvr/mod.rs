mod props;
use alvr_common::{once_cell::sync::Lazy, parking_lot::Mutex, OptLazy};
use alvr_packets::Haptics;
pub use props::*;

use crate::{
    input_mapping, logging_backend, ServerCoreContext, ServerCoreEvent, SERVER_DATA_MANAGER,
};
use std::{
    ffi::{c_char, c_void},
    thread,
    time::Duration,
};

static SERVER_CORE_CONTEXT: OptLazy<ServerCoreContext> = Lazy::new(|| {
    logging_backend::init_logging();

    Mutex::new(Some(ServerCoreContext::new()))
});

extern "C" fn driver_ready_idle(set_default_chap: bool) {
    thread::spawn(move || {
        unsafe { crate::InitOpenvrClient() };

        if set_default_chap {
            // call this when inside a new thread. Calling this on the parent thread will crash
            // SteamVR
            unsafe {
                crate::SetChaperoneArea(2.0, 2.0);
            }
        }

        if let Some(context) = &*SERVER_CORE_CONTEXT.lock() {
            context.start_connection();
        }

        loop {
            let event = if let Some(context) = &*SERVER_CORE_CONTEXT.lock() {
                match context.poll_event() {
                    Some(event) => event,
                    None => {
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
            } else {
                break;
            };

            match event {
                ServerCoreEvent::ClientConnected => {
                    unsafe {
                        crate::InitializeStreaming();
                        crate::RequestDriverResync();
                    };
                }
                ServerCoreEvent::ClientDisconnected => unsafe { crate::DeinitializeStreaming() },
                ServerCoreEvent::ShutdownPending => {
                    SERVER_CORE_CONTEXT.lock().take();

                    unsafe { crate::ShutdownSteamvr() };
                }
                ServerCoreEvent::RestartPending => {
                    if let Some(context) = SERVER_CORE_CONTEXT.lock().take() {
                        context.restart();
                    }

                    unsafe { crate::ShutdownSteamvr() };
                }
            }
        }

        unsafe { crate::ShutdownOpenvrClient() };
    });
}

extern "C" fn send_haptics(device_id: u64, duration_s: f32, frequency: f32, amplitude: f32) {
    if let Some(context) = &*SERVER_CORE_CONTEXT.lock() {
        let haptics = Haptics {
            device_id,
            duration: Duration::from_secs_f32(f32::max(duration_s, 0.0)),
            frequency,
            amplitude,
        };

        context.send_haptics(haptics);
    }
}

extern "C" fn wait_for_vsync() {
    if SERVER_DATA_MANAGER
        .read()
        .settings()
        .video
        .optimize_game_render_latency
    {
        let context_lock = SERVER_CORE_CONTEXT.lock();

        if let Some(wait_duration) = context_lock
            .as_ref()
            .and_then(|ctx| ctx.duration_until_next_vsync())
        {
            thread::sleep(wait_duration);
        }
    }
}

pub extern "C" fn shutdown_driver() {
    SERVER_CORE_CONTEXT.lock().take();
}

/// This is the SteamVR/OpenVR entry point
/// # Safety
#[no_mangle]
pub unsafe extern "C" fn HmdDriverFactory(
    interface_name: *const c_char,
    return_code: *mut i32,
) -> *mut c_void {
    // Make sure the context is initialized, and initialize logging
    SERVER_CORE_CONTEXT.lock().as_ref();

    crate::DriverReadyIdle = Some(driver_ready_idle);
    crate::GetSerialNumber = Some(get_serial_number);
    crate::SetOpenvrProps = Some(set_device_openvr_props);
    crate::RegisterButtons = Some(input_mapping::register_buttons);
    crate::HapticsSend = Some(send_haptics);
    crate::ShutdownRuntime = Some(shutdown_driver);
    crate::WaitForVSync = Some(wait_for_vsync);

    crate::CppOpenvrEntryPoint(interface_name, return_code)
}
