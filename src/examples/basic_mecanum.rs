use std::sync::atomic::Ordering;
use std::time::Instant;
use blaze_ftc::control::gamepad::Gamepad;
use blaze_ftc::control::hardware::Direction;
use blaze_ftc::control::MotorPIDF::MotorPIDF;
use blaze_ftc::control::robot::{GamepadHandler, Robot};
use blaze_ftc::threads::send::SEND_SATURATION;
use crate::examples::mecanum_with_brake_pid_mode::{ControlMode, MBPStateUpdate, MBPTarget};

pub fn robot_init_mecanum(robot: &mut Robot<MTarget, MStateUpdate>) -> MTarget {
    //this is how you log. info and above are logged, trace is reserved for debugging serialization
    //and internals
    log::info!("initing!");
    //pretty self-explanatory, note that we do not have support for motor names. idk if that's a
    //good thing I'll let you handle that
    robot.hub_0.set_direction(1, Direction::Backwards);
    robot.hub_0.set_direction(3, Direction::Backwards);
    //give it something implementing GamepadHandler w/ the right types or generics.
    //this function is backed by a list, so send as many and as many types as you want.
    //for instance, you might want to reuse the saturation/looptime logger.
    robot.add_gp_handler(SimpleMecanumGamepadHandler {last_ran: Instant::now()});
    //you have to return the initial target.
    MTarget {}
}

//placeholder
#[derive(Clone, PartialEq, Debug)]
pub struct MTarget {}
//also placeholder! We don't have any state to store
#[derive(PartialEq, Clone, Debug)]
pub enum MStateUpdate {}

pub struct SimpleMecanumGamepadHandler {
    /*if we to store state for this handler, this is where you put it*/
    last_ran: Instant
}
impl GamepadHandler<MTarget, MStateUpdate> for SimpleMecanumGamepadHandler {
    fn update(&mut self, robot: &Robot<MTarget, MStateUpdate>, gp0: &Gamepad, gp2: &Gamepad) {
        let s = 0.7;
        let y = -gp0.left_stick_y;
        let x = gp0.left_stick_x;
        let turn = gp0.right_stick_x;
        //note, you will have to change the motors yourself. eventually i will make this
        //more configurable in the init function, but for now only directions are
        robot.hub_0.send_motor_command(0, (y + x + turn) * s);
        robot.hub_0.send_motor_command(1, (y - x - turn) * s);
        robot.hub_0.send_motor_command(2, (y - x + turn) * s);
        robot.hub_0.send_motor_command(3, (y + x - turn) * s);

        //not strictly needed, but we might as well log write saturation and loop times.
        //note that gamepad looptimes are determined by how fast java gives us new gamepads,
        //which is artificially slow because otherwise we saturate the UART line.
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        robot.telemetry.add_string("loop time", &format!("{} ms", self.last_ran.elapsed().subsec_millis()));

        self.last_ran = Instant::now();
    }
}