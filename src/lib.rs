mod examples;

use std::sync::atomic::{AtomicBool, Ordering};
use blaze_ftc::control::gamepad::Gamepad;
use blaze_ftc::control::hardware::{Direction, LynxHub};
use blaze_ftc::control::robot::{Robot, KILL_CHANNEL};
use blaze_ftc::crossbeam_channel::{unbounded, Receiver, Sender};
use blaze_ftc::JNI_OnLoad_handler;
use blaze_ftc::serialization::command_utils::Module;
use blaze_ftc::serialization::packet::Packet;
use blaze_ftc::telemetry::telemetry::Telemetry;
use jni::sys::jint;
use crate::examples::auto_pids::robot_init_auto;
use crate::examples::basic_mecanum::robot_init_mecanum;
use crate::examples::mecanum_with_brake_pid_mode::robot_init_modes;
use crate::examples::neutrino_proxy::robot_init_neutrino;

#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(vm: jni::JavaVM, _: *mut std::ffi::c_void) -> jint {
    log::info!("got jni onload!");
    //this is the function called when the dll is loaded
    //you need it so the rest of the jni functions know what to do when the opmode starts
    JNI_OnLoad_handler(initfunc)
}
pub fn initfunc(hub_0: &'static LynxHub, hub_1: Option<&'static LynxHub>,
                gp_receiver: &'static Receiver<(Vec<u8>, Vec<u8>)>, telemetry: &'static Telemetry, to_run: i32) -> () {
    log::info!("initfunc ran! to_run:{}", to_run);
    //this is the function called when an opmode is actually started. to_run is passed through from the opmode config object
    match to_run {
        0 => Robot::new(hub_0, hub_1, gp_receiver, telemetry, robot_init_neutrino).init(),
        1 => Robot::new(hub_0, hub_1, gp_receiver, telemetry, robot_init_mecanum).init(),
        2 => Robot::new(hub_0, hub_1, gp_receiver, telemetry, robot_init_modes).init(),
        3 => Robot::new(hub_0, hub_1, gp_receiver, telemetry, robot_init_auto).init(),
        _ => {
            run_bare(hub_0, hub_1, gp_receiver, telemetry, to_run);
        }
    };
}

//here is how you can run without the little framework I built. For simple teleops, this is fine tbh.
//The Robot framework is designed mostly for autos anyway. However, if you want concurrency, it will
//be much more difficult to write it this way. probably. look you can do what you want ima be honest with you i came up with the idea of this like a week and a half ago :sob:
fn run_bare(hub_0: &'static LynxHub, hub_1: Option<&'static LynxHub>,
            gamepad_in: &'static Receiver<(Vec<u8>, Vec<u8>)>, telemetry: &'static Telemetry, opmode_to_run: i32) {
    let mut gp = Gamepad::new();
    hub_0.set_direction(1, Direction::Backwards);
    hub_0.set_direction(3, Direction::Backwards);
    //kill channel gets () written to it if the opmode needs to stop
    while KILL_CHANNEL.get_or_init(|| unbounded()).1.try_recv().is_err() {
        let gp_data = gamepad_in.recv().expect("gamepad wait failed");
        gp.read_into(gp_data.0.as_slice());

        let s = 0.7;
        let y = -gp.left_stick_y;
        let x = gp.left_stick_x;
        let turn = gp.right_stick_x;
        hub_0.send_motor_command(0, (y + x + turn) * s);
        hub_0.send_motor_command(1, (y - x - turn) * s);
        hub_0.send_motor_command(2, (y - x + turn) * s);
        hub_0.send_motor_command(3, (y + x - turn) * s);
    }
}