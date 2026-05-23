use std::any::Any;
use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::{Duration, Instant};
use blaze_ftc::control::hardware::Direction;
use blaze_ftc::control::MotorPIDF::MotorPIDF;
use blaze_ftc::control::robot::{BulkReadHandler, MainThread, Robot, ThreadSafe};
use blaze_ftc::serialization::lynx_commands::lynx_commands::LynxGetBulkDataResponseData;
use blaze_ftc::threads::send::SEND_SATURATION;

pub fn robot_init_auto(robot: &mut Robot) {
    log::info!("initing!");
    robot.hub_0.set_direction(1, Direction::Backwards);
    robot.hub_0.set_direction(3, Direction::Backwards);
    robot.add_hub_0_handler(AutoHub0Handler { pids: [MotorPIDF::new(0.008, 0.000004, 0.0, 0.0025); 4], last_ran: Instant::now() });
    robot.set_main_thread(main_thread);
    robot.hub_0.send_bulk_read();
    sleep(Duration::from_millis(1));
    robot.hub_0.send_bulk_read();
}
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MotorTargets {
    //you *should* use an array for this but I'm just illustrating how state works so whatever
    m0_target: Option<i32>,
    m1_target: Option<i32>,
    m2_target: Option<i32>,
    m3_target: Option<i32>
}
#[derive(Clone, Debug, PartialEq)]
pub struct MotorStatusUpdate {
    m0: i32,
    m1: i32,
    m2: i32,
    m3: i32,
}
//so for internal type gymnastics reasons you need to impl UnsafeAnyExt for all your
// state and target structs. Do it like this:
use blaze_ftc::control::robot::unsafe_any::UnsafeAnyExt;
unsafe impl UnsafeAnyExt for MotorStatusUpdate {}
unsafe impl UnsafeAnyExt for MotorTargets {}

fn main_thread(thread: &mut MainThread) {
    thread.telemetry.add_string("stage", "0");//telemetry, obviously
    let mut target_pos = MotorTargets {
        m0_target: None,
        m1_target: None,
        m2_target: None,
        m3_target: None,
    };
    //put target sends our struct to the handlers, so just drop a copy in there,
    //you can edit target_pos and send it again.
    thread.put_target(target_pos.clone());
    log::info!("was able to get data? {:?}", thread.get_target::<MotorTargets>());

    sleep(Duration::from_secs(3));

    log::info!("waited");

    //send our targets. this is like, an example auto so obviously the code is a bit verbose.
    //in an actual situation you'd probably send a target pose or something instead of made up positions
    target_pos.m0_target = Some(1000);
    target_pos.m2_target = Some(1500);
    thread.put_target(target_pos.clone());//actually send the target

    log::info!("was able to get data?2 {:?}", thread.get_target::<MotorTargets>());

    //the "wait_for_status" function will keep calling the closure you pass it until that closure
    //returns true. You can access both local scope variables and the &mut MainThread to do your
    //comparisons. Be careful not to unwrap/expect anything here if possible
    thread.wait_for_status(|mt| {
        if let Some(data) = mt.get_target::<MotorStatusUpdate>() {
            data.near(&target_pos, 25)
        } else {false}
    });
    log::info!("waited for status!");

    thread.telemetry.add_string("stage", "1");

    target_pos.m0_target = None;
    target_pos.m2_target = None;
    target_pos.m1_target = Some(-1000);
    target_pos.m3_target = Some(-1500);
    thread.put_target(target_pos.clone());//send the new target struct
    //now, the previous way of waiting was more powerful but more verbose, you can also
    //so it this way, where we specify the one type we want to look at:
    thread.wait_for_status_type(|_mt, data: Box<MotorStatusUpdate>| data.near(&target_pos, 25));

    thread.telemetry.add_string("stage", "2");

    //you can get the updated values of your state without the wait function very easily
    let status: Option<Box<MotorStatusUpdate>> = thread.get_status();
    if let Some(status) = status {
        thread.telemetry.add_i64("final motor 0 pos", status.m0 as i64);
        thread.telemetry.add_i64("final motor 1 pos", status.m1 as i64);
    }
}
struct AutoHub0Handler {
    pids: [MotorPIDF; 4],
    last_ran: Instant
}
impl BulkReadHandler for AutoHub0Handler {
    fn update(&mut self, robot: &Robot, data: &LynxGetBulkDataResponseData) {
        let target: Option<MotorTargets> = robot.target::<MotorTargets>();//this method has a clone for concurrency reasons.
        //best practice is to set it to a variable, and this makes accessing it less repetitive.

        robot.hub_0.send_bulk_read();//send it first, resending the data request before the commands increases speed

        //the from bits thing is because there is no AtomicF64
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        let elapsed = self.last_ran.elapsed();
        robot.telemetry.add_string("loop time", &format!("{} ms, {} micros",elapsed.subsec_millis(), elapsed.subsec_micros()));

        self.last_ran = Instant::now();

        robot.telemetry.add_bool("got targets", target.is_some());
        let target = if let Some(it) = target {
            it
        } else { return; };
        //sorry for the messy code. this is by no means the best way to do this.
        let targets = [
            target.m0_target,
            target.m1_target,
            target.m2_target,
            target.m3_target
        ];

        for i in 0..self.pids.len() {
            if targets[i].is_none() {
                robot.hub_0.send_motor_command(i as u8, 0.0);
                continue
            }
            self.pids[i].set_target(targets[i].unwrap() as f32);
            let pos = data.motors[i].position as f32;
            let cmd = self.pids[i].update(pos);
            robot.telemetry.add_f64(&format!("motor {} target", i), self.pids[i].get_target() as f64);
            robot.telemetry.add_i64(&format!("motor {} pos", i), pos as i64);
            robot.telemetry.add_f64(&format!("motor {} power", i), cmd as f64);
            robot.hub_0.send_motor_command(i as u8, cmd);
        }
        robot.send_state_update(MotorStatusUpdate {
            m0: data.motors[0].position,
            m1: data.motors[1].position,
            m2: data.motors[2].position,
            m3: data.motors[3].position,
        });

    }
}

//boilerplate functions. these are needed because i did the enum wierd, don't make the motor id
//represented by multiple enum options
impl MotorStatusUpdate {
    fn near(&self, target: &MotorTargets, tolerance: i32) -> bool {
        let pairs = [(self.m0, target.m0_target), (self.m1, target.m1_target),
            (self.m2, target.m2_target), (self.m3, target.m3_target)];
        for (pos, target) in pairs {
            if let Some(value) = target {
                if (pos - value).abs() > tolerance.abs() {
                    return false;
                }
            }
        }
        true
    }
}