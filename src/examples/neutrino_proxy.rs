use std::sync::atomic::Ordering;
use blaze_ftc::control::hardware::DO_MOTOR_CACHING;
use blaze_ftc::control::robot::{Robot, SdkPacketHandler};
use blaze_ftc::crossbeam_channel::Sender;
use blaze_ftc::serialization::command::Command;
use blaze_ftc::serialization::command::Command::Ack;
use blaze_ftc::serialization::commands::AckData;
use blaze_ftc::serialization::lynx_commands::base_lynx_command::LynxCommand;
use blaze_ftc::serialization::packet::Packet;
use blaze_ftc::threads::send::SEND_SATURATION;

pub fn robot_init_neutrino(robot: &mut Robot<NeutrinoTarget, NeutrinoStateUpdate>) -> NeutrinoTarget {
    log::info!("initing!");
    DO_MOTOR_CACHING.store(false, Ordering::SeqCst);//THIS FOR TESTING
    robot.add_proxy_interceptor(
        MotorAckProxyInterceptor {}
    );
    NeutrinoTarget::default()
}
struct MotorAckProxyInterceptor {

}
impl SdkPacketHandler<NeutrinoTarget, NeutrinoStateUpdate> for MotorAckProxyInterceptor {
    fn handle_packet(&mut self, robot: &Robot<NeutrinoTarget, NeutrinoStateUpdate>, packet: Packet, to_reader: &Sender<Packet>) -> Option<Packet> {
        match &packet.payload_data {
            Command::LynxCommand(it) => {
                match &it.command {
                    LynxCommand::LynxSetMotorPowerCommand(it) => {
                        if let Some(target_sender) = self.try_get_sender(robot, packet.dest_module_addr) {
                            let old_msg_num = packet.message_number;
                            let ack: Command = Ack(AckData {attention_required: false});
                            let ack_packet = Packet::new_full(ack, 0, packet.dest_module_addr, old_msg_num, old_msg_num);
                            to_reader.send(ack_packet).unwrap();//send the ack directly to java
                            target_sender.send(packet).unwrap();//send on the motor command unchanged

                            let saturation = f64::from_bits(SEND_SATURATION.load(Ordering::SeqCst));
                            let modded = saturation * 100.0;
                            robot.telemetry.add_f64("write saturation", modded);

                            None
                        } else {Some(packet)}
                    },
                    _ => Some(packet)
                }
            }
            _ => Some(packet)
        }
        //Some(packet)
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct NeutrinoTarget {}
#[derive(PartialEq, Clone, Debug)]
pub enum NeutrinoStateUpdate {}
