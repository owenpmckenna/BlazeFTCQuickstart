//this is a trivial, kind of useless example, meant to show how state can be transferred between
//different handlers. you can use this method if you want, or you can use a static variable.
//if you use a static variable though, fixing concurrency bugs is your problem.

use std::sync::atomic::Ordering;
use blaze_ftc::control::gamepad::Gamepad;
use blaze_ftc::control::hardware::Direction;
use blaze_ftc::control::MotorPIDF::MotorPIDF;
use blaze_ftc::control::robot::{BulkReadHandler, GamepadHandler, MainThread, Robot};
use blaze_ftc::serialization::lynx_commands::lynx_commands::LynxGetBulkDataResponseData;
use blaze_ftc::threads::send::SEND_SATURATION;

pub fn robot_init_modes(robot: &mut Robot<MBPTarget, MBPStateUpdate>) -> MBPTarget {
    log::info!("initing!");
    robot.hub_0.set_direction(1, Direction::Backwards);
    robot.hub_0.set_direction(3, Direction::Backwards);
    robot.add_gp_handler(MecanumGamepadHandler { a_was_pressed: false });
    robot.add_hub_0_handler(PidController { motor_target: false, pids: [MotorPIDF::new(0.008, 0.000004, 0.0, 0.0025); 4] });
    robot.add_update_processor(main_processor);
    MBPTarget {ctrl_mode: ControlMode::Mecanum }
}
fn main_processor(thread: &mut MainThread<MBPTarget, MBPStateUpdate>, state_update: &MBPStateUpdate) {
    //this is a simple processor. it will see updates before the main thread, which is designed to
    //block waiting for things, which is why when all we need to do is "echo" it's a good option
    match state_update {
        MBPStateUpdate::Mode(it) => {
            thread.target = it.clone();//set the target. the target in the MainThread is not the same
            //as the one in the Robot. you have to send it.
            thread.set_target();//actually send it to the Robot instance
        }
    }
}
//this target is the struct exposed to handlers
//also, sorry for the naming, but these have to be accessed outside the crate which leads to
//name collisions with the other opmodes. if anyone has a fix, I'd love to hear it
#[derive(Clone, PartialEq, Debug)]
pub struct MBPTarget {
    ctrl_mode: ControlMode
}
//status update is the type you can send back to the control thread. in this case it pretty much
//just contains a target. again, you could also use a static mutex or something
#[derive(PartialEq, Clone, Debug)]
pub enum MBPStateUpdate {
    Mode(MBPTarget)
}
//our enum defining state
#[derive(PartialEq, Clone, Debug)]
pub enum ControlMode {
    Brake,
    Mecanum
}

struct MecanumGamepadHandler {
    a_was_pressed: bool
}
impl GamepadHandler<MBPTarget, MBPStateUpdate> for MecanumGamepadHandler {
    fn update(&mut self, robot: &Robot<MBPTarget, MBPStateUpdate>, gp0: &Gamepad, gp1: &Gamepad) {
        let a_pressed = gp0.a && !self.a_was_pressed;
        self.a_was_pressed = gp0.a;
        let s = 0.7;
        let y = -gp0.left_stick_y;
        let x = gp0.left_stick_x;
        let turn = gp0.right_stick_x;
        if robot.target().ctrl_mode == ControlMode::Mecanum {
            //log::info!("mecanum! x:{} y:{} turn:{}", x, y, turn);
            robot.hub_0.send_motor_command(0, (y + x + turn) * s);
            robot.hub_0.send_motor_command(1, (y - x - turn) * s);
            robot.hub_0.send_motor_command(2, (y - x + turn) * s);
            robot.hub_0.send_motor_command(3, (y + x - turn) * s);

            if a_pressed {
                robot.hub_0.send_bulk_read();//kick off bulk reads. they don't happen unless you request them
                //the only other place triggering bulk reads here is the bulk read handler
                //so someone has to trigger them first
                robot.send_state_update(MBPStateUpdate::Mode(MBPTarget { ctrl_mode: ControlMode::Brake} ))
            }
        } else {
            if a_pressed {
                //this ofc will be reflected back at us
                robot.send_state_update(MBPStateUpdate::Mode(MBPTarget { ctrl_mode: ControlMode::Mecanum} ))
            }
        }

        //the write thread tells us the current write saturation. if this goes over 100%, the lynx module crashes.
        //I plan to create a kind of "scheduler" later but for now this is your job haha
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        robot.telemetry.add_string("mode", &format!("{:?}", robot.target().ctrl_mode));
    }
}

//this is how we are allowed to store mutable state for each handler
struct PidController {
    motor_target: bool,
    pids: [MotorPIDF; 4]
}
impl BulkReadHandler<MBPTarget, MBPStateUpdate> for PidController {
    fn update(&mut self, robot: &Robot<MBPTarget, MBPStateUpdate>, data: &LynxGetBulkDataResponseData) {
        if robot.target().ctrl_mode == ControlMode::Brake {
            match &self.motor_target {
                true => {
                    for i in 0..self.pids.len() {
                        let cmd = self.pids[i].update(data.motors[i].position as f32);
                        //telemetry!
                        robot.telemetry.add_string(&format!("motor {} target", i), &format!("{}", self.pids[i].get_target()));
                        robot.telemetry.add_string(&format!("motor {} power", i), &format!("{}", cmd));
                        robot.hub_0.send_motor_command(i as u8, cmd);
                    }
                }
                false => {
                    for i in 0..self.pids.len() {
                        self.pids[i].set_target(data.motors[i].position as f32)
                    }
                    self.motor_target = true;
                }
            }
            //resend bulk read so we get to run again
            robot.hub_0.send_bulk_read();
        } else {
            //make sure we know that the targets are "stale"
            self.motor_target = false;
        }
    }
}
