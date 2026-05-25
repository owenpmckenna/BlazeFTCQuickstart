use blaze_ftc::control::MotorPIDF::{MotorPIDF, PIDF};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Instant, SystemTime};
use blaze_ftc::control::gamepad::Gamepad;
use blaze_ftc::control::hardware::{LynxHub, DO_MOTOR_CACHING, MOTOR_CACHING_THRESHOLD};
use blaze_ftc::control::java_closure_system::JNICrossPinpointHandler;
use blaze_ftc::control::robot::{BulkReadHandler, GamepadHandler, MainThread, Robot, SdkPacketHandler, ThreadSafe};
use blaze_ftc::crossbeam_channel::Sender;
use blaze_ftc::sdk_proxy::proxy::TIMING_TRACKER;
use blaze_ftc::serialization::command::Command;
use blaze_ftc::serialization::command::Command::{Ack};
use blaze_ftc::serialization::commands::AckData;
use blaze_ftc::serialization::lynx_commands::base_lynx_command::{LynxCommand, LynxCommandData};
use blaze_ftc::serialization::lynx_commands::base_lynx_command::LynxCommand::LynxGetMotorChannelModeResponse;
use blaze_ftc::serialization::lynx_commands::lynx_commands::{DcMotorRunMode, LynxGetBulkDataResponseData, LynxGetMotorChannelModeResponseData};
use blaze_ftc::serialization::lynx_commands::lynx_commands::DcMotorRunMode::RunWithoutEncoder;
use blaze_ftc::serialization::packet::Packet;
use blaze_ftc::threads::send::SEND_SATURATION;

pub fn robot_init_neutrino(robot: &mut Robot) {
    log::info!("initing!");
    DO_MOTOR_CACHING.store(false, Ordering::SeqCst);
    MOTOR_CACHING_THRESHOLD.store(1, Ordering::SeqCst);
    robot.add_hub_0_handler(NeutrinoBRHandler::new(robot.hub_0));
    if let Some(hub_1) = robot.hub_1 {
        robot.add_hub_1_handler(NeutrinoBRHandler::new(hub_1))
    }
    robot.add_proxy_interceptor_hub_0(
        MotorAckProxyInterceptor {}
    );
    robot.add_proxy_interceptor_hub_1(
        MotorAckProxyInterceptor {}
    );
    robot.add_update_processor(process_update);
    JNICrossPinpointHandler::put_on_robot(robot);
    //if you want to do performance testing, enable the neutrino gp handler
    //by uncommenting the next line. It lets you enable and disable some stuff at runtime
    // robot.add_gp_handler(NeutrinoGamepadHandler {});
}
static USE_NEUTRINO: OnceLock<AtomicBool> = OnceLock::new();
static USE_NEUTRINO_EXTRAS: OnceLock<AtomicBool> = OnceLock::new();
fn do_extras() -> bool {
    USE_NEUTRINO.get_or_init(|| AtomicBool::new(true)).load(Ordering::SeqCst)
}
struct NeutrinoGamepadHandler {}
impl GamepadHandler for NeutrinoGamepadHandler {
    fn update(&mut self, robot: &Robot, gp0: &Gamepad, gp1: &Gamepad) {
        if gp1.dpad_left {
            robot.telemetry.add_string("neutrino enabled", "true");
            USE_NEUTRINO.get_or_init(|| AtomicBool::new(true)).store(true, Ordering::SeqCst);
        } else if gp1.dpad_right {
            robot.telemetry.add_string("neutrino enabled", "false");
            USE_NEUTRINO.get_or_init(|| AtomicBool::new(false)).store(false, Ordering::SeqCst);
        }
        if gp1.dpad_up {
            robot.telemetry.add_string("neutrino extras enabled", "true");
            USE_NEUTRINO_EXTRAS.get_or_init(|| AtomicBool::new(true)).store(true, Ordering::SeqCst);
        } else if gp1.dpad_down {
            robot.telemetry.add_string("neutrino extras enabled", "false");
            USE_NEUTRINO_EXTRAS.get_or_init(|| AtomicBool::new(false)).store(false, Ordering::SeqCst);
        }
    }
}
struct MotorAckProxyInterceptor {

}
impl MotorAckProxyInterceptor {
    fn send_ack_to_sdk(packet: &Packet, to_reader: &Sender<Packet>) {
        let old_msg_num = packet.message_number;
        let ack: Command = Ack(AckData {attention_required: false});
        let ack_packet = Packet::new_full(ack, 0, packet.dest_module_addr, old_msg_num, old_msg_num);
        log::trace!("neutrino: responding with packet: msg_num:{}, dest_module:{}", old_msg_num, packet.dest_module_addr);
        to_reader.send(ack_packet).unwrap();//send the ack directly to java
    }
}
impl SdkPacketHandler for MotorAckProxyInterceptor {
    fn handle_packet(&mut self, robot: &Robot, packet: Packet, to_reader: &Sender<Packet>) -> Option<Packet> {
        log::trace!("neutrino: handling packet. ref:{}, msg:{}, addr:{}, command:{}", packet.reference_number, packet.message_number, packet.dest_module_addr, packet.payload_data);
        let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
        let modded = saturation * 100.0;
        robot.telemetry.add_f64("write saturation", modded);
        robot.telemetry.add_String("neutrino write timing", TIMING_TRACKER.to_text());
        if !USE_NEUTRINO.get_or_init(|| AtomicBool::new(true)).load(Ordering::SeqCst) {
            //return Some(packet)
        }
        if let Command::LynxCommand(_) = packet.payload_data {
            return self.handle_lynx_command(robot, packet, to_reader)
        }
        Some(packet)
    }
}
impl MotorAckProxyInterceptor {
    fn handle_lynx_command(&mut self, robot: &Robot, packet: Packet, to_reader: &Sender<Packet>) -> Option<Packet> {
        let it = if let Command::LynxCommand(it) = &packet.payload_data {
            it//uhhhhh this is stupid. idk.
        } else { panic!("type gymnastics failed. this should never happen.") };
        match &it.command {
            LynxCommand::LynxSetMotorPowerCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) {
                    target_sender.send_motor_command_i16(it.motor, it.power);
                    Self::send_ack_to_sdk(&packet, to_reader);
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxSetServoPulseWidthCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) {
                    Self::send_ack_to_sdk(&packet, to_reader);
                    target_sender.send_packet(packet); //send on the command unchanged
                    None
                } else {Some(packet)}
            },
            //these commented out because sometimes I2C relies on processing NACKs correctly and
            //this may cause problems for end users.
            /*LynxCommand::LynxI2CSingleByteWriteCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    Self::send_ack_to_sdk(&packet, to_reader);
                    target_sender.send_packet(packet); //send on the command unchanged
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxI2cWriteMultipleBytesCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    Self::send_ack_to_sdk(&packet, to_reader);
                    target_sender.send_packet(packet); //send on the command unchanged
                    None
                } else {Some(packet)}
            },*/
            LynxCommand::LynxSetMotorChannelModeCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    Self::send_ack_to_sdk(&packet, to_reader);
                    let not_already_running = target_sender.get_motor_mode(0) == RunWithoutEncoder &&
                        target_sender.get_motor_mode(1) == RunWithoutEncoder &&
                        target_sender.get_motor_mode(2) == RunWithoutEncoder &&
                        target_sender.get_motor_mode(3) == RunWithoutEncoder;
                    log::info!("Setting motor channel... id: {}, motor: {}, mode: {:?}, not_already_running: {}", packet.dest_module_addr, it.motor, it.run_mode, not_already_running);
                    target_sender.set_behavior(it.motor, RunWithoutEncoder, it.zero_power_behavior);
                    target_sender.set_motor_mode_inner_(it.motor, it.run_mode);//we now handle pidf in blaze!
                    if not_already_running && it.run_mode != RunWithoutEncoder {//we have to trigger these the first time
                        target_sender.send_bulk_read();
                    }
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxGetMotorChannelModeCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    let old_msg_num = packet.message_number;
                    let data = LynxGetMotorChannelModeResponseData {
                        run_mode: target_sender.get_motor_mode(it.motor),
                        zero_power_behavior: target_sender.get_zero_power_behavior(it.motor),
                    };
                    let command = LynxGetMotorChannelModeResponse(data);
                    let resp = Command::LynxCommand(LynxCommandData { module: &target_sender.module, command });
                    let response = Packet::new_full(resp, 0, packet.dest_module_addr, old_msg_num, old_msg_num);
                    log::trace!("neutrino: forwarding get motor channel {}, mode:{:?}, zph:{:?}", it.motor, data.zero_power_behavior, data.run_mode);
                    to_reader.send(response).unwrap(); //send the response directly to java
                    log::trace!("fake responded the motor mode!");
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxSetMotorPIDFCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    let id = if target_sender.module.is_parent { 0 } else { 1 };
                    robot.send_state_update(NeutrinoStateUpdate::PIDFUpdate(id, it.motor, it.to_pidf()));
                    Self::send_ack_to_sdk(&packet, to_reader);
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxSetMotorVelocityTargetCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    let id = if target_sender.module.is_parent { 0 } else { 1 };
                    log::trace!("ordered to fire Vel update! m:{},v:{},i:{}", it.motor, it.velocity, id);
                    robot.send_state_update(NeutrinoStateUpdate::VelUpdate(id, it.motor, it.velocity));
                    Self::send_ack_to_sdk(&packet, to_reader);
                    None
                } else {Some(packet)}
            },
            LynxCommand::LynxSetMotorTargetPositionCommand(it) => {
                if let Some(target_sender) = self.try_get_hub(robot, packet.dest_module_addr) && do_extras() {
                    let id = if target_sender.module.is_parent { 0 } else { 1 };
                    log::info!("ordered to fire Pos update! m:{},p:{},i:{}", it.motor, it.position, id);
                    robot.send_state_update(NeutrinoStateUpdate::PosUpdate(id, it.motor, it.position));
                    Self::send_ack_to_sdk(&packet, to_reader);
                    None
                } else {Some(packet)}
            },
            _ => Some(packet)
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct NeutrinoTarget {
    pids: [[PIDF; 4]; 2],
    vel_targets: [[i16; 4]; 2],
    pos_targets: [[i32; 4]; 2],
}
#[derive(PartialEq, Clone, Debug)]
pub enum NeutrinoStateUpdate {
    PIDFUpdate(u8, u8, PIDF),//hub, motor, data
    VelUpdate(u8, u8, i16),
    PosUpdate(u8, u8, i32)
}
pub fn process_update(mt: &mut MainThread, update: &Box<dyn ThreadSafe>) {
    if let Some(update) = update.as_any().downcast_ref::<NeutrinoStateUpdate>() {
        if let Some(mut target) = mt.get_target::<NeutrinoTarget>().cloned() {
            match update {
                NeutrinoStateUpdate::PIDFUpdate(h, m, pid) => { target.pids[*h as usize][*m as usize] = *pid; }
                NeutrinoStateUpdate::VelUpdate(h, m, vel) => { target.vel_targets[*h as usize][*m as usize] = *vel; }
                NeutrinoStateUpdate::PosUpdate(h, m, pos) => { target.pos_targets[*h as usize][*m as usize] = *pos; }
            }
            mt.put_target(target);
            log::info!("got pidf update! update: {:?}", update);
        }
    }
}
struct NeutrinoBRHandler {
    index: usize,
    hub_id: u8,
    hub: &'static LynxHub,
    motors: [MotorPIDF; 4],
    last_update: Instant
}
impl NeutrinoBRHandler {
    fn new(hub: &'static LynxHub) -> NeutrinoBRHandler {
        NeutrinoBRHandler {
            index: 0,
            hub_id: if hub.module.is_parent {0} else {1},
            hub,
            motors: [MotorPIDF::new(0.0, 0.0, 0.0, 0.0); 4],
            last_update: Instant::now()
        }
    }
}
impl BulkReadHandler for NeutrinoBRHandler {
    fn update(&mut self, robot: &Robot, data: &LynxGetBulkDataResponseData) {
        let mut needs_update = false;
        let target = robot.target::<NeutrinoTarget>();
        if let Some(target) = target {
            for i in 0..4 {
                let updated = self.motors[i].maybe_update_pids(&target.pids[self.hub_id as usize][i]);
                match self.hub.get_motor_mode(i as u8) {
                    RunWithoutEncoder => {}
                    DcMotorRunMode::RunUsingEncoder => {
                        let vel = data.motors[i].velocity;
                        let target = target.vel_targets[self.hub_id as usize][i];
                        self.motors[i].set_target(target as f32);
                        let speed = self.motors[i].update(vel as f32);
                        log::trace!("rue: {}, vel: {}, upd: {}, target: {}", speed, vel, updated, target);
                        self.hub.send_motor_command(i as u8, speed);
                        needs_update = true;
                    }
                    DcMotorRunMode::RunToPosition => {
                        let pos = data.motors[i].position;
                        self.motors[i].set_target(target.pos_targets[self.hub_id as usize][i] as f32);
                        let speed = self.motors[i].update(pos as f32);
                        self.hub.send_motor_command(i as u8, speed);
                        needs_update = true;
                    }
                };
            }
        }
        if needs_update {
            let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
            let modded = saturation * 100.0;
            log::trace!("sending update from brh, ws:{}, ind:{}!", modded, self.index);
            self.index += 1;
            self.hub.send_bulk_read();
        } else {
            log::info!("no longer need to update pidfs.")
        }
        let elapsed = self.last_update.elapsed();
        robot.telemetry.add_f64("neutrino pidf loop hz", 1_000_000.0 / elapsed.subsec_micros() as f64);
        self.last_update = Instant::now()
    }
}