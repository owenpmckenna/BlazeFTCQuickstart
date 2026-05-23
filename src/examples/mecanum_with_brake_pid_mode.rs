//this is a trivial, kind of useless example, meant to show how state can be transferred between
//different handlers. you can use this method if you want, or you can use a static variable.
//if you use a static variable though, fixing concurrency bugs is your problem.

use std::sync::atomic::Ordering;
use blaze_ftc::control::gamepad::Gamepad;
use blaze_ftc::control::hardware::Direction;
use blaze_ftc::control::MotorPIDF::MotorPIDF;
use blaze_ftc::control::robot::{BulkReadHandler, GamepadHandler, MainThread, Robot, ThreadSafe};
use blaze_ftc::serialization::lynx_commands::lynx_commands::LynxGetBulkDataResponseData;
use blaze_ftc::threads::send::SEND_SATURATION;

pub fn robot_init_modes(robot: &mut Robot) {
    log::info!("initing!");
    robot.hub_0.set_direction(1, Direction::Backwards);
    robot.hub_0.set_direction(3, Direction::Backwards);
    robot.add_gp_handler(MecanumGamepadHandler { a_was_pressed: false });
    robot.add_hub_0_handler(PidController { motor_target: false, pids: [MotorPIDF::new(0.008, 0.000004, 0.0, 0.0025); 4] });
    robot.add_update_processor(main_processor);
    robot.set_main_thread(|it| it.put_target(ControlMode::Mecanum))
}
fn main_processor(thread: &mut MainThread, state_update: &Box<dyn ThreadSafe>) {
    //this is a simple processor. it will see updates before the main thread, which is designed to
    //block waiting for things, which is why when all we need to do is "echo" it's a good option
    if let Some(it) = state_update.as_any().downcast_ref::<ControlMode>() {
        //just throw it back into the target if it's a MBPTarget
        thread.put_target(it.clone())
    }
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
impl GamepadHandler for MecanumGamepadHandler {
    fn update(&mut self, robot: &Robot, gp0: &Gamepad, _gp1: &Gamepad) {
        let a_pressed = gp0.a && !self.a_was_pressed;
        self.a_was_pressed = gp0.a;
        let s = 0.7;
        let y = -gp0.left_stick_y;
        let x = gp0.left_stick_x;
        let turn = gp0.right_stick_x;
        if let Some(ctrl_mode) = robot.target::<ControlMode>() && ctrl_mode == ControlMode::Mecanum {
            //log::info!("mecanum! x:{} y:{} turn:{}", x, y, turn);
            robot.hub_0.send_motor_command(0, (y + x + turn) * s);
            robot.hub_0.send_motor_command(1, (y - x - turn) * s);
            robot.hub_0.send_motor_command(2, (y - x + turn) * s);
            robot.hub_0.send_motor_command(3, (y + x - turn) * s);

            if a_pressed {
                robot.hub_0.send_bulk_read();//kick off bulk reads. they don't happen unless you request them
                //the only other place triggering bulk reads here is the bulk read handler
                //so someone has to trigger them first
                robot.send_state_update(ControlMode::Brake)
            }
        } else {
            if a_pressed {
                //this ofc will be reflected back at us
                robot.send_state_update(ControlMode::Mecanum)
            }
        }

        //the write thread tells us the current write saturation. if this goes over 100%, the lynx module crashes.
        //I plan to create a kind of "scheduler" later but for now this is your job haha
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        if let Some(mode) = robot.target::<ControlMode>() {
            robot.telemetry.add_string("mode", &format!("{:?}", mode));
        }
    }
}

//this is how we are allowed to store mutable state for each handler
struct PidController {
    motor_target: bool,
    pids: [MotorPIDF; 4]
}
impl BulkReadHandler for PidController {
    fn update(&mut self, robot: &Robot, data: &LynxGetBulkDataResponseData) {
        if let Some(mode) = robot.target::<ControlMode>() && mode == ControlMode::Brake {
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
