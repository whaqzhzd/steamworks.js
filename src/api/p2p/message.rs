use serde::{Deserialize, Serialize};
pub const HSTEAM_NET_CONNECTION_INVALID: u32 = 0;
/// https://partner.steamgames.com/doc/api/steamnetworkingtypes#ESteamNetConnectionEnd
pub const ESTEAM_NET_CONNECTION_END_APP_MIN: i32 = 1000;

// 网络协议定义
#[derive(PartialEq, Eq, Debug)]
pub enum EMessage {
    Error = -1,

    KEmsgBegin = 0,
    // 来自于服务器的信息
    KEmsgServer = 1,
    // 来自于客户端的信息
    KEmsgClient = 2,

    // 服务器信息
    KEmsgServerBegin = 300,
    KEmsgServerSendInfo = EMessage::KEmsgServerBegin as isize + 1,
    KEmsgServerFailAuthentication = EMessage::KEmsgServerBegin as isize + 2,
    KEmsgServerPassAuthentication = EMessage::KEmsgServerBegin as isize + 3,
    KEmsgServerAllReadyToGo = EMessage::KEmsgServerBegin as isize + 4,
    KEmsgServerFrameData = EMessage::KEmsgServerBegin as isize + 5,
    KEmsgServerFramesData = EMessage::KEmsgServerBegin as isize + 6,
    KEmsgServerGameStart = EMessage::KEmsgServerBegin as isize + 7,
    KEmsgServerSetGameStartDataComplete = EMessage::KEmsgServerBegin as isize + 8,
    KEmsgServerBroadcast = EMessage::KEmsgServerBegin as isize + 9,

    // 客户端信息
    KEmsgClientBegin = 500,
    KEmsgClientBeginAuthentication = EMessage::KEmsgClientBegin as isize + 2,
    KEmsgClientLoadComplete = EMessage::KEmsgClientBegin as isize + 3,
    KEmsgClientFrameData = EMessage::KEmsgClientBegin as isize + 4,
    KEmsgClientBroadcast = EMessage::KEmsgClientBegin as isize + 5,

    // P2P认证信息
    KEmsgP2pbegin = 600,

    // 语音聊天
    KEmsgVoiceChatBegin = 700,

    KEforceDword = 0x7fffffff,
}

enum EDisconnectReason {
    EDRClientDisconnect = ESTEAM_NET_CONNECTION_END_APP_MIN as isize + 1,
    EDRServerClosed = ESTEAM_NET_CONNECTION_END_APP_MIN as isize + 2,
    EDRServerReject = ESTEAM_NET_CONNECTION_END_APP_MIN as isize + 3,
    EDRServerFull = ESTEAM_NET_CONNECTION_END_APP_MIN as isize + 4,
    EDRClientKicked = ESTEAM_NET_CONNECTION_END_APP_MIN as isize + 5,
}

impl From<EMessage> for isize {
    fn from(msg: EMessage) -> Self {
        msg as isize
    }
}

impl From<EMessage> for Vec<u8> {
    fn from(msg: EMessage) -> Self {
        (msg as i32).to_le_bytes().to_vec()
    }
}

impl Into<EMessage> for Vec<u8> {
    fn into(self) -> EMessage {
        let bytes: [u8; 4] = self.try_into().unwrap();
        let id = i32::from_le_bytes(bytes);

        match id {
            x if x == EMessage::KEmsgBegin as i32 => EMessage::KEmsgBegin,
            x if x == EMessage::KEmsgServer as i32 => EMessage::KEmsgServer,
            x if x == EMessage::KEmsgClient as i32 => EMessage::KEmsgClient,
            x if x == EMessage::KEmsgServerBegin as i32 => EMessage::KEmsgServerBegin,
            x if x == EMessage::KEmsgServerSendInfo as i32 => EMessage::KEmsgServerSendInfo,
            x if x == EMessage::KEmsgServerFailAuthentication as i32 => {
                EMessage::KEmsgServerFailAuthentication
            }
            x if x == EMessage::KEmsgServerPassAuthentication as i32 => {
                EMessage::KEmsgServerPassAuthentication
            }
            x if x == EMessage::KEmsgServerAllReadyToGo as i32 => EMessage::KEmsgServerAllReadyToGo,
            x if x == EMessage::KEmsgServerFrameData as i32 => EMessage::KEmsgServerFrameData,
            x if x == EMessage::KEmsgServerFramesData as i32 => EMessage::KEmsgServerFramesData,
            x if x == EMessage::KEmsgServerGameStart as i32 => EMessage::KEmsgServerGameStart,
            x if x == EMessage::KEmsgServerSetGameStartDataComplete as i32 => {
                EMessage::KEmsgServerSetGameStartDataComplete
            }
            x if x == EMessage::KEmsgServerBroadcast as i32 => EMessage::KEmsgServerBroadcast,
            x if x == EMessage::KEmsgClientBegin as i32 => EMessage::KEmsgClientBegin,
            x if x == EMessage::KEmsgClientBeginAuthentication as i32 => {
                EMessage::KEmsgClientBeginAuthentication
            }
            x if x == EMessage::KEmsgClientLoadComplete as i32 => EMessage::KEmsgClientLoadComplete,
            x if x == EMessage::KEmsgClientFrameData as i32 => EMessage::KEmsgClientFrameData,
            x if x == EMessage::KEmsgClientBroadcast as i32 => EMessage::KEmsgClientBroadcast,
            x if x == EMessage::KEmsgP2pbegin as i32 => EMessage::KEmsgP2pbegin,
            x if x == EMessage::KEmsgVoiceChatBegin as i32 => EMessage::KEmsgVoiceChatBegin,
            _ => EMessage::Error,
        }
    }
}

pub trait INetMessage {
    const ID: EMessage;
}

macro_rules! NetMessage {
    ($T:ident, $enum_pattern: expr) => {
        impl INetMessage for $T {
            const ID: EMessage = $enum_pattern;
        }
    };
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerSendInfo {
    pub ul_steam_idserver: u64,
    pub is_vacsecure: bool,
    pub rgch_server_name: String,
}
NetMessage!(MsgServerSendInfo, EMessage::KEmsgServerSendInfo);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgClientBeginAuthentication {
    pub rgch_token: Vec<u8>,
}

NetMessage!(
    MsgClientBeginAuthentication,
    EMessage::KEmsgClientBeginAuthentication
);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerFailAuthentication;

NetMessage!(
    MsgServerFailAuthentication,
    EMessage::KEmsgServerFailAuthentication
);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgClientLoadComplete;

NetMessage!(MsgClientLoadComplete, EMessage::KEmsgClientLoadComplete);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgClientFrameData {
    pub types: u32,
    pub data: Vec<u8>,
}

NetMessage!(MsgClientFrameData, EMessage::KEmsgClientFrameData);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerFrameData {
    pub types: u32,
    pub data: Vec<u8>,
    pub local_steam_id: u64,
}

NetMessage!(MsgServerFrameData, EMessage::KEmsgServerFrameData);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgClientDataBroadcast {
    pub types: u32,
    pub data: Vec<u8>,
    pub local_steam_id: u64,
}

NetMessage!(MsgClientDataBroadcast, EMessage::KEmsgClientBroadcast);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerDataBroadcast {
    pub types: u32,
    pub data: Vec<u8>,
    pub local_steam_id: u64,
}

NetMessage!(MsgServerDataBroadcast, EMessage::KEmsgServerBroadcast);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerGameStart {
    pub game_data: Vec<MsgServerFrameData>,
    pub buffer_size: u32,
}
NetMessage!(MsgServerGameStart, EMessage::KEmsgServerGameStart);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerFramesData {
    pub game_data: Vec<MsgServerFrameData>,
    pub buffer_size: u32,
    pub frame_id: u32,
}
NetMessage!(MsgServerFramesData, EMessage::KEmsgServerFramesData);

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct MsgServerPassAuthentication {
    pub player_position: u32,
}
NetMessage!(
    MsgServerPassAuthentication,
    EMessage::KEmsgServerPassAuthentication
);
