use std::sync::atomic::Ordering;
use std::thread::{sleep, Thread};
use std::time::{Duration, Instant};
use blaze_ftc::control::hardware::Direction;
use blaze_ftc::control::MotorPIDF::MotorPIDF;
use blaze_ftc::control::robot::{BulkReadHandler, MainThread, Robot};
use blaze_ftc::serialization::lynx_commands::lynx_commands::LynxGetBulkDataResponseData;
use blaze_ftc::threads::send::SEND_SATURATION;
use crate::examples::auto_pids::MotorStatusUpdate::*;
use crate::examples::basic_mecanum::MStateUpdate;

pub fn robot_init_auto(robot: &mut Robot<MotorTargets, MotorStatusUpdate>) -> MotorTargets {
    log::info!("initing!");
    robot.hub_0.set_direction(1, Direction::Backwards);
    robot.hub_0.set_direction(3, Direction::Backwards);
    robot.add_hub_0_handler(AutoHub0Handler { pids: [MotorPIDF::new(0.008, 0.000004, 0.0, 0.0025); 4], last_ran: Instant::now() });
    robot.set_main_thread(main_thread);
    robot.hub_0.send_bulk_read();
    sleep(Duration::from_millis(1));
    robot.hub_0.send_bulk_read();
    MotorTargets {
        m0_target: None,
        m1_target: None,
        m2_target: None,
        m3_target: None,
    }
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MotorTargets {
    //you *should* use an array for this but I'm just illustrating how state works so whatever
    m0_target: Option<i32>,
    m1_target: Option<i32>,
    m2_target: Option<i32>,
    m3_target: Option<i32>
}
#[derive(Clone, Debug)]
pub enum MotorStatusUpdate {
    //should probably be: MStatus{motor: u8, pos: i32} but I want to show how we use discriminants
    //to store multiple types of state. we match based on the enum type (M[0-3]Status) but not its value
    M0Status{pos: i32},
    M1Status{pos: i32},
    M2Status{pos: i32},
    M3Status{pos: i32}
}
fn main_thread(thread: &mut MainThread<MotorTargets, MotorStatusUpdate>) {
    thread.telemetry.add_string("stage", "0");//telemetry, obviously

    //ok, so because of rust reasons, you need references to all these things so you can get the
    // updated reference. see the end of the function for what that looks like
    let mut m0 = M0Status {pos: 0};//starting positions
    let mut m1 = M1Status {pos: 0};
    let mut m2 = M2Status {pos: 0};
    let mut m3 = M3Status {pos: 0};

    //send our targets. this is like, an example auto so obviously the code is a bit verbose.
    //in an actual situation you'd probably send a target pose or something instead of made up positions
    thread.target.m0_target = Some(1000);
    thread.target.m1_target = Some(1500);
    m0 = m0.set_pos(1000);
    m1 = m1.set_pos(1500);
    thread.set_target();//actually send the target

    //this will block until the status updates reflect the target updates you gave it.
    //idk if this is the best way to do it. actionable suggestions are appreciated.
    thread.wait_for_status(&[&m0, &m1], &[]);

    thread.telemetry.add_string("stage", "1");

    thread.target.m0_target = None;
    thread.target.m1_target = None;
    thread.target.m2_target = Some(-1000);
    thread.target.m3_target = Some(-1500);
    m2 = m2.set_pos(-1000);
    m3 = m3.set_pos(-1500);
    thread.set_target();//actually send the target
    thread.wait_for_status(&[&m2, &m3], &[]);

    thread.telemetry.add_string("stage", "2");

    //you can get the updated values of your state enum by passing in the old value.
    //the discriminant will be used to grab the new value from the cache, if any.
    //if anyone knows how to give access to the internal enum value instead (I'm guessing
    //that'd require a macro) please tell me
    if let Some(M0Status { pos }) = thread.get_updated_status(&m0) {
        thread.telemetry.add_i64("final motor 0 pos", *pos as i64)
    }
    //this also works
    if let M1Status { pos } = thread.get_updated_status(&M1Status {pos: 0}).unwrap() {
        thread.telemetry.add_i64("final motor 1 pos", *pos as i64)
    }
}
struct AutoHub0Handler {
    pids: [MotorPIDF; 4],
    last_ran: Instant
}
impl BulkReadHandler<MotorTargets, MotorStatusUpdate> for AutoHub0Handler {
    fn update(&mut self, robot: &Robot<MotorTargets, MotorStatusUpdate>, data: &LynxGetBulkDataResponseData) {
        let target = robot.target();//this method has a clone for concurrency reasons.
        //best practice is to set it to a variable, and this makes accessing it less repetitive.

        //sorry for the messy code. this is by no means the best way to do this.
        let targets = [
            target.m0_target,
            target.m1_target,
            target.m2_target,
            target.m3_target
        ];

        robot.hub_0.send_bulk_read();//send it first, might make it faster idk
        for i in 0..self.pids.len() {
            if targets[i].is_none() {
                continue
            }
            self.pids[i].set_target(targets[i].unwrap() as f32);
            let pos = data.motors[i].position as f32;
            let cmd = self.pids[i].update(pos);
            robot.telemetry.add_f64(&format!("motor {} target", i), self.pids[i].get_target() as f64);
            robot.telemetry.add_i64(&format!("motor {} pos", i), pos as i64);
            robot.telemetry.add_f64(&format!("motor {} power", i), cmd as f64);
            robot.send_state_update(MotorStatusUpdate::from_id(i, pos as i32));
            robot.hub_0.send_motor_command(i as u8, cmd);
        }

        //the from bits thing is because there is no AtomicF64
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        let elapsed = self.last_ran.elapsed();
        robot.telemetry.add_string("loop time", &format!("{} ms, {} micros",elapsed.subsec_millis(), elapsed.subsec_micros()));

        self.last_ran = Instant::now();
    }
}

//boilerplate functions. these are needed because i did the enum wierd, don't make the motor id
//represented by multiple enum options
impl MotorStatusUpdate {
    const fn set_pos(&mut self, new_pos: i32) -> MotorStatusUpdate {
        match self {
            M0Status { .. } => {M0Status {pos: new_pos}}
            M1Status { .. } => {M1Status {pos: new_pos}}
            M2Status { .. } => {M2Status {pos: new_pos}}
            M3Status { .. } => {M3Status {pos: new_pos}}
        }
    }
    const fn pos(&self) -> i32 {
        match self {
            M0Status { pos } => {*pos}
            M1Status { pos } => {*pos}
            M2Status { pos } => {*pos}
            M3Status { pos } => {*pos}
        }
    }
    const fn from_id(motor: usize, pos: i32) -> MotorStatusUpdate {
        match motor {
            0 => M0Status {pos},
            1 => M1Status {pos},
            2 => M2Status {pos},
            3 => M3Status {pos},
            _ => {panic!("wrong id!")}
        }
    }
}
impl PartialEq for MotorStatusUpdate {
    fn eq(&self, other: &Self) -> bool {
        (self.pos() - other.pos()).abs() < 35
    }
}