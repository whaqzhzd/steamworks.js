use napi_derive::napi;

#[napi]
pub mod steamp2p {
    use crate::api::p2p::message::*;
    use crate::client::now;
    use napi::bindgen_prelude::ToNapiValue;
    use napi::bindgen_prelude::*;
    use napi::{
        threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
        JsFunction,
    };
    use networking_sockets::*;
    use std::collections::{HashMap, HashSet};
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::net::Ipv4Addr;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use steamworks::networking_types::{ListenSocketEvent, SendFlags};
    use steamworks::networking_types::{NetConnectionEndReason, NetworkingIdentity};
    use steamworks::networking_utils::NetworkingUtils;
    use steamworks::{ServerManager, SteamServersConnected, *};
    use steamworks::{SteamError, SteamId};

    #[napi]
    pub struct Handle {
        handle: Option<steamworks::CallbackHandle<ServerManager>>,
    }

    impl PartialEq for Handle {
        fn eq(&self, other: &Handle) -> bool {
            self.handle.as_ref().unwrap().id() == other.handle.as_ref().unwrap().id()
        }
    }

    impl Eq for Handle {}

    impl Hash for Handle {
        fn hash<H>(&self, state: &mut H)
        where
            H: Hasher,
        {
            self.handle.as_ref().unwrap().id().hash(state);
        }
    }

    #[napi]
    impl Handle {
        #[napi]
        pub fn disconnect(&mut self) {
            self.handle = None;
        }
    }

    #[napi]
    #[derive(PartialEq, Eq)]
    pub enum EServerMode {
        EServerModeInvalid = 0,                 // DO NOT USE
        EServerModeNoAuthentication = 1, // Don't authenticate user logins and don't list on the server list
        EServerModeAuthentication = 2, // Authenticate users, list on the server list, don't run VAC on clients that connect
        EServerModeAuthenticationAndSecure = 3, // Authenticate users, list on the server list and VAC protect clients
    }

    #[napi]
    #[derive(PartialEq, Eq)]
    pub enum EServerGameState {
        KEserverWaitingForPlayers,
        KEserverActive,
        KEserverDraw,
        KEserverWinner,
        KEserverExiting,
    }

    #[napi]
    pub struct SteamServerConnectFailure {
        /// The reason we failed to connect to the Steam servers
        pub reason: i64,
        /// Whether we are still retrying the connection.
        pub still_retrying: bool,
    }

    #[napi]
    pub struct SteamServersDisconnected {
        pub reason: i64,
    }

    #[napi]
    pub struct ValidateAuthTicketResponse {
        /// The steam id of the entity that provided the ticket
        pub steam_id: BigInt,
        /// The result of the validation
        pub response: Option<i32>,
        /// The steam id of the owner of the game. Differs from
        /// `steam_id` if the game is borrowed.
        pub owner_steam_id: BigInt,
    }

    #[napi]
    pub struct GSPolicyResponseCallback {
        pub secure: u8,
    }

    struct ClientConnectionData {
        active: bool,
        load_complete: bool,
        ul_tick_count_last_data: i64,
        steam_iduser: NetworkingIdentity,
        hsteam_net_connection: Option<NetConnection<ServerManager>>,
    }

    impl ClientConnectionData {
        pub fn new(
            identity: NetworkingIdentity,
            connection: Option<NetConnection<ServerManager>>,
        ) -> Self {
            ClientConnectionData {
                active: false,
                load_complete: false,
                ul_tick_count_last_data: 0,
                steam_iduser: identity,
                hsteam_net_connection: connection,
            }
        }
    }

    #[napi(js_name = "SteamServer")]
    pub struct JsSteamServer {
        is_connected_to_steam: bool,
        policy_response_callback: bool,
        can_close: bool,
        setp: bool,
        dt_total: i32,
        interval: f64,
        app_id: u32,
        /// 游戏状态
        game_state: EServerGameState,
        /// 已经链接到服务器的客户端数据
        rg_client_data: Vec<ClientConnectionData>,
        /// 正在等待验证的客户端数据
        rg_pending_client_data: Vec<ClientConnectionData>,
        /// 当前房间最多玩家数
        max_players: u8,
        name: String,
        map_name: String,
        server_name: String,
        server_id: u64,
        player_bot: u32,
        lobby_id: u64,
        /// 第几次链接成功
        connected_success_count: u32,
        player_count: u32,

        pch_game_dir: String,
        un_ip: u32,
        us_steam_port: u16,
        us_game_port: u16,
        us_query_port: u16,
        server_mode: EServerMode,
        pch_version_string: String,

        listen_socket: Option<ListenSocket<ServerManager>>, // drop CloseListenSocket
        net_poll_group: Option<NetPollGroup<ServerManager>>, // drop DestroyPollGroup
        server_raw: Option<Server>,                         // drop LogOff Shutdown
        server_single: Option<SingleClient<ServerManager>>,
        server_sockets: Option<NetworkingSockets<ServerManager>>,

        utils: Option<NetworkingUtils<ServerManager>>,
        handle: Option<HashSet<Handle>>,
        send: Option<Sender<SteamServerEvent>>,

        frame_messages: HashMap<u64, Vec<MsgServerFrameData>>,
        frame_messages_size: u32,
        game_start_data: Option<MsgServerGameStart>,
    }

    #[napi]
    pub struct SteamServerManager {
        rx: Receiver<SteamServerEvent>,
        raw: JsSteamServer,

        steam_servers_connected: Option<ThreadsafeFunction<i32, ErrorStrategy::Fatal>>,
        steam_server_connect_failure:
            Option<ThreadsafeFunction<SteamServerConnectFailure, ErrorStrategy::Fatal>>,
        steam_servers_disconnected:
            Option<ThreadsafeFunction<SteamServersDisconnected, ErrorStrategy::Fatal>>,
        all_ready_to_go: Option<ThreadsafeFunction<i32, ErrorStrategy::Fatal>>,
        gspolicy_response_callback: Option<ThreadsafeFunction<(), ErrorStrategy::Fatal>>,
    }

    #[napi]
    impl SteamServerManager {
        #[napi(ts_args_type = "callback: (count:number) => void")]
        pub fn on_servers_connected(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<i32, ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();
            self.steam_servers_connected = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_servers_connected");
        }

        #[napi(
            ts_args_type = "callback: ({reason,stillRetrying}:{reason:number,stillRetrying:boolean}) => void"
        )]
        pub fn on_servers_connect_failure(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<
                SteamServerConnectFailure,
                ErrorStrategy::Fatal,
            > = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();

            self.steam_server_connect_failure = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("steam_server_connect_failure");
        }

        #[napi(ts_args_type = "callback: ({reason}:{reason:number}) => void")]
        pub fn on_servers_disconnected(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<
                SteamServersDisconnected,
                ErrorStrategy::Fatal,
            > = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();

            self.steam_servers_disconnected = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_servers_disconnected");
        }

        #[napi(ts_args_type = "callback: (count:number) => void")]
        pub fn on_all_ready_to_go(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<i32, ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();

            self.all_ready_to_go = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_all_ready_to_go");
        }

        #[napi(ts_args_type = "callback: () => void")]
        pub fn on_gspolicy_response_callback(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<(), ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();

            self.gspolicy_response_callback = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_gspolicy_response_callback");
        }

        pub fn receive(&mut self) {
            let mut server = &mut self.raw;

            loop {
                if let Some(socket) = server.listen_socket.as_ref() {
                    if let Some(event) = socket.try_receive_event() {
                        #[cfg(feature = "dev")]
                        dbg!("ListenSocketEvent Receive");

                        match event {
                            ListenSocketEvent::Connecting(mut request) => {
                                #[cfg(feature = "dev")]
                                dbg!("ListenSocketEvent::Connecting");

                                if server.rg_pending_client_data.len() >= server.max_players.into()
                                {
                                    #[cfg(feature = "dev")]
                                    dbg!("Rejecting connection; server full");

                                    request.reject(
                                        NetConnectionEndReason::NetConnectionEnd(
                                            networking_types::NetConnectionEnd::AppException,
                                        ),
                                        Some("Server full!"),
                                    );
                                    return;
                                }

                                let remote = request.remote();
                                let find = server
                                    .rg_pending_client_data
                                    .iter()
                                    .find(|f| {
                                        f.steam_iduser.steam_id().unwrap()
                                            == remote.steam_id().unwrap()
                                    })
                                    .or_else(|| {
                                        server.rg_client_data.iter().find(|f| {
                                            f.steam_iduser.steam_id().unwrap()
                                                == remote.steam_id().unwrap()
                                        })
                                    });
                                if find.is_some() {
                                    return;
                                }

                                if let Err(_) = request.accept() {
                                    #[cfg(feature = "dev")]
                                    dbg!("ConnectionRequest::Accept Error");

                                    request.reject(
                                        NetConnectionEndReason::NetConnectionEnd(
                                            networking_types::NetConnectionEnd::AppException,
                                        ),
                                        Some("Failed to accept connection"),
                                    );

                                    return;
                                }

                                let pending = ClientConnectionData::new(remote, None);
                                server.rg_pending_client_data.push(pending);
                            }
                            ListenSocketEvent::Connected(connected) => {
                                #[cfg(feature = "dev")]
                                dbg!("ListenSocketEvent::Connected");

                                let remote = connected.remote();

                                let find = server.rg_pending_client_data.iter().position(|f| {
                                    f.steam_iduser.steam_id().unwrap() == remote.steam_id().unwrap()
                                });

                                if let Some(f) = find {
                                    let request = connected.connection();
                                    request.set_poll_group(server.net_poll_group.as_ref().unwrap());

                                    let msg = MsgServerSendInfo {
                                        ul_steam_idserver: server.server_id,
                                        is_vacsecure: server.server_raw.as_ref().unwrap().secure(),
                                        rgch_server_name: server.server_name.clone(),
                                    };

                                    #[cfg(feature = "dev")]
                                    dbg!("ListenSocketEvent::Connected::send_message MsgServerSendInfo");

                                    server.send_message(msg, request);
                                    server
                                        .rg_pending_client_data
                                        .get_mut(f)
                                        .unwrap()
                                        .hsteam_net_connection = Some(connected.take_connection());
                                } else {
                                    #[cfg(feature = "dev")]
                                    dbg!("ListenSocketEvent::Connected take_connection().close");

                                    connected.take_connection().close(
                                        NetConnectionEndReason::NetConnectionEnd(
                                            networking_types::NetConnectionEnd::AppException,
                                        ),
                                        Some("can not find rg_pending_client_data"),
                                        false,
                                    );
                                    return;
                                }
                            }
                            ListenSocketEvent::Disconnected(disconnected) => {
                                #[cfg(feature = "dev")]
                                dbg!("ListenSocketEvent::Disconnected");

                                let remote = disconnected.remote();

                                server
                                    .rg_client_data
                                    .iter()
                                    .position(|f| {
                                        f.steam_iduser.steam_id().unwrap()
                                            == remote.steam_id().unwrap()
                                    })
                                    .map(|f| {
                                        let data = server.rg_client_data.get_mut(f).unwrap();
                                        data.hsteam_net_connection.take().unwrap().close(
                                            NetConnectionEndReason::NetConnectionEnd(
                                                networking_types::NetConnectionEnd::AppGeneric,
                                            ),
                                            None,
                                            false,
                                        );

                                        server.rg_client_data.remove(f);
                                    });
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            loop {
                if let Ok(result) = self.rx.try_recv() {
                    match result {
                        SteamServerEvent::SteamServersConnected(_) => {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServerEvent::SteamServersConnected");

                            server.is_connected_to_steam = true;
                            server.connected_success_count += 1;
                            server.send_updated_server_details_to_steam();

                            if let Some(fun) = self.steam_servers_connected.as_ref() {
                                fun.call(
                                    server.connected_success_count as i32,
                                    ThreadsafeFunctionCallMode::Blocking,
                                );
                            }
                        }
                        SteamServerEvent::SteamServerConnectFailure(failure) => {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServerEvent::SteamServerConnectFailure");

                            server.is_connected_to_steam = false;
                            if let Some(fun) = self.steam_server_connect_failure.as_ref() {
                                fun.call(failure, ThreadsafeFunctionCallMode::Blocking);
                            }
                        }
                        SteamServerEvent::SteamServersDisconnected(disconnected) => {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServerEvent::SteamServersDisconnected");

                            server.is_connected_to_steam = false;
                            if let Some(fun) = self.steam_servers_disconnected.as_ref() {
                                fun.call(disconnected, ThreadsafeFunctionCallMode::Blocking);
                            }
                        }
                        SteamServerEvent::ValidateAuthTicketResponse(response) => {
                            let index = server.rg_pending_client_data.iter().position(|data| {
                                if data.active {
                                    data.steam_iduser.steam_id().unwrap()
                                        == SteamId::from_raw(response.steam_id.get_u64().1)
                                } else {
                                    return false;
                                }
                            });

                            if let Some(pending_auth_index) = index {
                                if response.response.is_none() {
                                    println!("auth completed for a client");
                                } else {
                                    println!("auth failed for a client");
                                };

                                if server.on_auth_completed(
                                    response.response.is_none(),
                                    pending_auth_index,
                                ) {
                                    if let Some(fun) = self.all_ready_to_go.as_ref() {
                                        fun.call(1, ThreadsafeFunctionCallMode::Blocking);
                                    }
                                }
                            }
                        }
                        SteamServerEvent::GSPolicyResponseCallback(_) => {
                            if let Some(raw) = server.server_raw.as_ref() {
                                if raw.secure() {
                                    println!("server is val secure");
                                } else {
                                    println!("server is not vac secure");
                                }

                                println!("server steam id is : {:?}", raw.steam_id().raw());
                                server.policy_response_callback = true;

                                if let Some(fun) = self.gspolicy_response_callback.as_ref() {
                                    fun.call((), ThreadsafeFunctionCallMode::Blocking);
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        #[napi]
        pub fn receive_network_data(&mut self) {
            if self.raw.listen_socket.as_ref().is_none() && !self.raw.is_connected_to_steam() {
                return;
            }

            if let Some(group) = self.raw.net_poll_group.as_ref() {
                for e in group.receive_messages(128) {
                    let remote = e.identity_peer().steam_id().unwrap();
                    let data = e.data();

                    if data.len() < 4 {
                        println!("got garbage on server socket, too short");

                        drop(e); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                        continue;
                    }

                    let header: EMessage = data[0..4].to_vec().into();
                    let body = &data[4..];

                    if header == EMessage::Error {
                        drop(e); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                        continue;
                    }

                    match header {
                        EMessage::KEmsgClientFrameData => {
                            if let Ok(msg) = rmps::from_slice::<MsgClientFrameData>(body) {
                                if msg.types == 0 {
                                    self.raw.on_client_games_data(msg, remote);
                                } else {
                                    self.raw.on_client_frame_data(msg, remote);
                                };
                            }
                        }
                        EMessage::KEmsgClientBroadcast => {
                            if let Ok(msg) = rmps::from_slice::<MsgClientDataBroadcast>(body) {
                                self.raw.on_client_broadcast(msg);
                            }
                        }
                        EMessage::KEmsgClientBeginAuthentication => {
                            if let Ok(msg) = rmps::from_slice::<MsgClientBeginAuthentication>(body)
                            {
                                self.raw.on_client_begin_authentication(msg, remote);
                            }
                        }
                        EMessage::KEmsgClientLoadComplete => {
                            if self.raw.game_state == EServerGameState::KEserverActive {
                                break;
                            }

                            let mut all_load = true;
                            self.raw.rg_client_data.iter_mut().for_each(|data| {
                                if data.steam_iduser.steam_id().unwrap() == remote {
                                    data.load_complete = true;
                                }

                                if !data.load_complete {
                                    all_load = false;
                                }
                            });

                            if all_load {
                                let take = self.raw.game_start_data.take().unwrap();

                                self.raw.rg_client_data.iter().for_each(|f| {
                                    self.raw.send_message_ref(
                                        &take,
                                        f.hsteam_net_connection.as_ref().unwrap(),
                                    );
                                });
                            }
                        }
                        _ => panic!("Bad client info msg,{:?}", header),
                    }

                    drop(e); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                }
            }
        }

        #[napi]
        pub fn run_callbacks(&mut self) {
            self.receive();

            {
                self.raw.run_callbacks();
            }

            self.receive_network_data();
        }

        #[napi]
        pub fn is_connected_to_steam(&self) -> bool {
            self.raw.is_connected_to_steam
        }

        #[napi]
        pub fn is_policy_response_callback(&self) -> bool {
            self.raw.policy_response_callback
        }

        /// 设置应用ID
        #[napi]
        pub fn set_appid(&mut self, appid: u32) {
            self.raw.set_appid(appid);
        }

        /// 可以加入一个服务器并同时游戏的最大玩家数量
        #[napi]
        pub fn set_max_player(&mut self, max: u8) {
            self.raw.set_max_player(max);
        }

        /// 设置应用名称
        #[napi]
        pub fn set_app_name(&mut self, name: String) {
            self.raw.set_app_name(name);
        }

        /// 设置地图名称
        #[napi]
        pub fn set_map_name(&mut self, name: String) {
            self.raw.set_map_name(name);
        }

        /// 设置服务器名称
        #[napi]
        pub fn set_server_name(&mut self, name: String) {
            self.raw.set_server_name(name);
        }

        /// 设置机器人的数量
        #[napi]
        pub fn set_bot_player_count(&mut self, bot: u32) {
            self.raw.set_bot_player_count(bot);
        }

        /// 设置FPS
        #[napi]
        pub fn set_interval(&mut self, interval: f64) {
            self.raw.set_interval(interval);
        }

        /// 获取游戏服务器的steam 唯一ID
        #[napi]
        pub fn get_server_steam_id(&self) -> u64 {
            self.raw.get_server_steam_id()
        }

        /// 设置当前服务器的大厅唯一ID
        #[napi]
        pub fn set_lobby_id(&mut self, lobby_id: BigInt) {
            self.raw.set_lobby_id(lobby_id);
        }

        ///  获取大厅唯一ID
        #[napi]
        pub fn get_lobby_id(&self) -> u64 {
            self.raw.lobby_id
        }

        /// 初始化参数
        ///
        /// `pch_game_dir` 游戏名称
        ///
        /// `un_ip` 您要绑定的 IP 地址。 （应使用主机序，即 127.0.0.1 == 0x7f000001）。 您可以使用 INADDR_ANY 绑定所有本地 IPv4 地址
        ///  
        /// `us_steam_port` 用于与 Steam 服务器通信的本地端口
        ///
        /// `us_game_port` 客户端进行游戏将连接至的端口
        ///
        /// `us_query_port` 将管理服务器浏览器相关任务以及来自客户端的 info ping 的端口
        ///
        /// `server_mode` 设置服务器的验证方法
        ///
        /// `pch_version_string` 版本字符串格式通常为 x.x.x.x，主服务器用它来检测服务器何时过期。 （只列出最新版的服务器）
        ///
        #[napi]
        pub fn initialize(
            &mut self,
            pch_game_dir: String,
            un_ip: u32,
            us_steam_port: u16,
            us_game_port: u16,
            us_query_port: u16,
            server_mode: EServerMode,
            pch_version_string: String,
        ) {
            self.raw.initialize(
                pch_game_dir,
                un_ip,
                us_steam_port,
                us_game_port,
                us_query_port,
                server_mode,
                pch_version_string,
            );
        }

        #[napi]
        pub fn open(&mut self) {
            self.raw.open();
        }
    }

    enum SteamServerEvent {
        SteamServersConnected(SteamServersConnected),
        SteamServerConnectFailure(SteamServerConnectFailure),
        SteamServersDisconnected(SteamServersDisconnected),
        ValidateAuthTicketResponse(ValidateAuthTicketResponse),
        GSPolicyResponseCallback(GSPolicyResponseCallback),
    }

    #[napi]
    pub fn create_async_server() -> SteamServerManager {
        let mut server = JsSteamServer::new();
        let (tx, rx) = channel();

        server.send = Some(tx);
        server.handle = Some(HashSet::new());

        #[cfg(feature = "dev")]
        dbg!("create_async_server");

        SteamServerManager {
            rx,
            raw: server,
            steam_servers_connected: None,
            steam_server_connect_failure: None,
            steam_servers_disconnected: None,
            all_ready_to_go: None,
            gspolicy_response_callback: None,
        }
    }

    #[napi]
    impl JsSteamServer {
        #[napi(constructor)]
        pub fn new() -> Self {
            let server = JsSteamServer {
                is_connected_to_steam: false,
                policy_response_callback: false,
                can_close: false,
                setp: false,
                dt_total: 0,
                interval: 0f64,
                app_id: 0,
                game_state: EServerGameState::KEserverExiting,
                rg_client_data: Vec::<ClientConnectionData>::new(),
                rg_pending_client_data: Vec::<ClientConnectionData>::new(),
                max_players: 0,
                name: String::from(""),
                map_name: String::from(""),
                server_name: String::from(""),
                server_id: 0,
                player_bot: 0,
                lobby_id: 0,
                connected_success_count: 0,
                player_count: 0,

                pch_game_dir: String::from(""),
                un_ip: 0,
                us_steam_port: 0,
                us_game_port: 0,
                us_query_port: 0,
                server_mode: EServerMode::EServerModeNoAuthentication,
                pch_version_string: String::from(""),

                server_raw: None,
                server_single: None,
                server_sockets: None,
                listen_socket: None,
                net_poll_group: None,

                utils: None,

                handle: None,
                send: None,

                frame_messages: HashMap::new(),
                frame_messages_size: 0,
                game_start_data: Some(MsgServerGameStart {
                    game_data: vec![],
                    buffer_size: 0,
                }),
            };

            server
        }

        #[napi]
        pub fn run_callbacks(&self) {
            if let Some(single) = self.server_single.as_ref() {
                single.run_callbacks();
            }
        }

        #[napi]
        pub fn send_updated_server_details_to_steam(&self) {
            if let Some(server) = self.server_raw.as_ref() {
                #[cfg(feature = "dev")]
                dbg!("send_updated_server_details_to_steam open");

                assert!(self.max_players != 0);

                server.set_max_players(self.max_players as i32);
                server.set_password_protected(false);
                server.set_server_name(&self.server_name);
                server.set_bot_player_count(self.player_bot as i32);
                server.set_map_name(&self.map_name);
            }
        }

        #[napi]
        pub fn is_connected_to_steam(&self) -> bool {
            self.is_connected_to_steam
        }

        /// 设置应用ID
        #[napi]
        pub fn set_appid(&mut self, appid: u32) {
            self.app_id = appid;
        }

        /// 可以加入一个服务器并同时游戏的最大玩家数量
        #[napi]
        pub fn set_max_player(&mut self, max: u8) {
            self.max_players = max;
        }

        /// 设置应用名称
        #[napi]
        pub fn set_app_name(&mut self, name: String) {
            self.name = name;
        }

        /// 设置地图名称
        #[napi]
        pub fn set_map_name(&mut self, name: String) {
            self.map_name = name;
        }

        /// 设置服务器名称
        #[napi]
        pub fn set_server_name(&mut self, name: String) {
            self.server_name = name;
        }

        /// 设置机器人的数量
        #[napi]
        pub fn set_bot_player_count(&mut self, bot: u32) {
            self.player_bot = bot;
        }

        /// 设置FPS
        #[napi]
        pub fn set_interval(&mut self, interval: f64) {
            self.interval = 1000.0 / interval / 1000.0;
        }

        /// 获取游戏服务器的steam 唯一ID
        #[napi]
        pub fn get_server_steam_id(&self) -> u64 {
            self.server_raw.as_ref().unwrap().steam_id().raw()
        }

        /// 设置当前服务器的大厅唯一ID
        #[napi]
        pub fn set_lobby_id(&mut self, lobby_id: BigInt) {
            self.lobby_id = lobby_id.get_u64().1;
        }

        ///  获取大厅唯一ID
        #[napi]
        pub fn get_lobby_id(&self) -> u64 {
            self.lobby_id
        }

        /// 初始化参数
        ///
        /// `pch_game_dir` 游戏名称
        ///
        /// `un_ip` 您要绑定的 IP 地址。 （应使用主机序，即 127.0.0.1 == 0x7f000001）。 您可以使用 INADDR_ANY 绑定所有本地 IPv4 地址
        ///  
        /// `us_steam_port` 用于与 Steam 服务器通信的本地端口
        ///
        /// `us_game_port` 客户端进行游戏将连接至的端口
        ///
        /// `us_query_port` 将管理服务器浏览器相关任务以及来自客户端的 info ping 的端口
        ///
        /// `server_mode` 设置服务器的验证方法
        ///
        /// `pch_version_string` 版本字符串格式通常为 x.x.x.x，主服务器用它来检测服务器何时过期。 （只列出最新版的服务器）
        ///
        #[napi]
        pub fn initialize(
            &mut self,
            pch_game_dir: String,
            un_ip: u32,
            us_steam_port: u16,
            us_game_port: u16,
            us_query_port: u16,
            server_mode: EServerMode,
            pch_version_string: String,
        ) {
            self.pch_game_dir = pch_game_dir;
            self.un_ip = un_ip;
            self.us_steam_port = us_steam_port;
            self.us_game_port = us_game_port;
            self.us_query_port = us_query_port;
            self.server_mode = server_mode;
            self.pch_version_string = pch_version_string;
        }

        #[napi]
        pub fn open(&mut self) {
            if self.server_raw.is_some() {
                return;
            }

            if self.lobby_id == 0 {
                return;
            }

            if self.max_players == 0 {
                return;
            }

            #[cfg(feature = "dev")]
            dbg!("steam server open entry");

            self.connected_success_count = 0;
            if let Ok((server, single)) = Server::init(
                Ipv4Addr::from(self.un_ip),
                self.us_steam_port,
                self.us_game_port,
                self.us_query_port,
                match self.server_mode {
                    EServerMode::EServerModeNoAuthentication => {
                        steamworks::ServerMode::NoAuthentication
                    }
                    EServerMode::EServerModeAuthentication => {
                        steamworks::ServerMode::Authentication
                    }
                    EServerMode::EServerModeAuthenticationAndSecure => {
                        steamworks::ServerMode::AuthenticationAndSecure
                    }
                    _ => panic!(),
                },
                self.pch_version_string.as_str(),
            ) {
                self.server_single = Some(single);
                self.server_sockets = Some(server.networking_server_sockets());
                self.utils = Some(server.networking_utils());
                self.server_raw = Some(server);

                if self.send.is_some() {
                    self.register();
                }
            } else {
                return;
            }

            if let Some(server) = self.server_raw.as_ref() {
                server.set_mod_dir(&self.pch_game_dir);
                server.set_product(&self.app_id.to_string());
                server.set_game_description(&self.name);
                server.log_on_anonymous();
                self.server_id = server.steam_id().raw();

                let client = crate::client::get_client();
                client.networking_utils().init_relay_network_access();

                if self.server_mode == EServerMode::EServerModeAuthenticationAndSecure {
                    server.enable_heartbeats(true);
                }

                #[cfg(feature = "dev")]
                dbg!("server init_relay_network_access success", self.server_id);
            }

            self.player_count = 0;
            self.game_state = EServerGameState::KEserverWaitingForPlayers;

            if let Some(sockets) = self.server_sockets.as_ref() {
                if let Ok(listen) = sockets.create_listen_socket_p2p(0, vec![]) {
                    #[cfg(feature = "dev")]
                    dbg!("server create_listen_socket_p2p success");

                    self.listen_socket = Some(listen);
                } else {
                    return;
                }
            } else {
                return;
            }

            if let Some(sockets) = self.server_sockets.as_ref() {
                self.net_poll_group = Some(sockets.create_poll_group());

                #[cfg(feature = "dev")]
                dbg!("server create_poll_group success");
            } else {
                return;
            }
            self.can_close = true;
        }

        #[napi(ts_args_type = "callback: () => void")]
        pub fn on_servers_connected(&self, handler: JsFunction) -> Handle {
            if let Some(server) = self.server_raw.as_ref() {
                let threadsafe_handler: ThreadsafeFunction<(), ErrorStrategy::Fatal> = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                Handle {
                    handle: Some(server.register_callback(move |_: SteamServersConnected| {
                        threadsafe_handler.call((), ThreadsafeFunctionCallMode::Blocking);
                    })),
                }
            } else {
                Handle { handle: None }
            }
        }

        #[napi(
            ts_args_type = "callback: ({reason,stillRetrying}:{reason:number,stillRetrying:boolean}) => void"
        )]
        pub fn on_servers_connect_failure(&self, handler: JsFunction) -> Handle {
            if let Some(server) = self.server_raw.as_ref() {
                let threadsafe_handler: ThreadsafeFunction<
                    SteamServerConnectFailure,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                Handle {
                    handle: Some(server.register_callback(
                        move |f: steamworks::SteamServerConnectFailure| {
                            threadsafe_handler.call(
                                SteamServerConnectFailure {
                                    reason: f.reason as i64,
                                    still_retrying: f.still_retrying,
                                },
                                ThreadsafeFunctionCallMode::Blocking,
                            );
                        },
                    )),
                }
            } else {
                Handle { handle: None }
            }
        }

        #[napi(ts_args_type = "callback: ({reason}:{reason:number}) => void")]
        pub fn on_servers_disconnected(&self, handler: JsFunction) -> Handle {
            if let Some(server) = self.server_raw.as_ref() {
                let threadsafe_handler: ThreadsafeFunction<
                    SteamServersDisconnected,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                Handle {
                    handle: Some(server.register_callback(
                        move |f: steamworks::SteamServersDisconnected| {
                            threadsafe_handler.call(
                                SteamServersDisconnected {
                                    reason: f.reason as i64,
                                },
                                ThreadsafeFunctionCallMode::Blocking,
                            );
                        },
                    )),
                }
            } else {
                Handle { handle: None }
            }
        }

        #[napi(
            ts_args_type = "callback: ({steamId,response,ownerSteamId}:{steamId:bigint,response:number,ownerSteamId:bigint}) => void"
        )]
        pub fn on_validate_auth_ticket_response(&self, handler: JsFunction) -> Handle {
            if let Some(server) = self.server_raw.as_ref() {
                let threadsafe_handler: ThreadsafeFunction<
                    ValidateAuthTicketResponse,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                Handle {
                    handle: Some(server.register_callback(
                        move |f: steamworks::ValidateAuthTicketResponse| {
                            threadsafe_handler.call(
                                ValidateAuthTicketResponse {
                                    steam_id: BigInt::from(f.steam_id.raw()),
                                    response: f.response.err().map(|e| e as i32),
                                    owner_steam_id: BigInt::from(f.owner_steam_id.raw()),
                                },
                                ThreadsafeFunctionCallMode::Blocking,
                            );
                        },
                    )),
                }
            } else {
                Handle { handle: None }
            }
        }

        #[napi(ts_args_type = "callback: ({secure}:{secure:boolean}) => void")]
        pub fn on_gspolicy_response_callback(&self, handler: JsFunction) -> Handle {
            if let Some(server) = self.server_raw.as_ref() {
                let threadsafe_handler: ThreadsafeFunction<
                    GSPolicyResponseCallback,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                Handle {
                    handle: Some(server.register_callback(
                        move |f: networking_utils::GSPolicyResponseCallback| {
                            threadsafe_handler.call(
                                GSPolicyResponseCallback { secure: f.secure },
                                ThreadsafeFunctionCallMode::Blocking,
                            );
                        },
                    )),
                }
            } else {
                Handle { handle: None }
            }
        }
    }

    impl JsSteamServer {
        pub fn on_client_broadcast(&mut self, msg: MsgClientDataBroadcast) {
            let server: MsgServerDataBroadcast = msg.into();
            self.rg_client_data.iter().for_each(|f| {
                self.send_message_ref(&server, f.hsteam_net_connection.as_ref().unwrap())
            });
        }

        pub fn on_client_frame_data(&mut self, msg: MsgClientFrameData, remote: SteamId) {
            let data = self
                .rg_client_data
                .iter_mut()
                .find(|f| f.steam_iduser.steam_id().unwrap() == remote);

            if let Some(_) = data.as_ref() {
                let values = self
                    .frame_messages
                    .entry(remote.raw())
                    .or_insert_with(|| vec![]);

                // 一个逻辑帧只接受同一个类型的同一个指令
                if let Some(d) = values.iter_mut().find(|v| v.types == msg.types) {
                    // 如果有了旧的数据
                    // 就用新数据覆盖
                    d.data = msg.data;
                } else {
                    // 如果没有旧数据
                    // 直接追加
                    values.push(msg.into());
                    self.frame_messages_size += 1;
                }
            }
        }

        pub fn on_client_games_data(&mut self, msg: MsgClientFrameData, remote: SteamId) {
            let data = self
                .rg_client_data
                .iter_mut()
                .find(|f| f.steam_iduser.steam_id().unwrap() == remote);

            if let Some(_) = data.as_ref() {
                let data = self.game_start_data.as_mut().unwrap();

                let values = data
                    .game_data
                    .iter_mut()
                    .find(|f| f.local_steam_id == remote.raw());

                // 一个逻辑帧只接受同一个类型的同一个指令
                if let Some(d) = values {
                    // 如果有了旧的数据
                    // 就用新数据覆盖
                    data.buffer_size -= d.data.len() as u32;
                    data.buffer_size += msg.data.len() as u32;
                    d.data = msg.data;
                } else {
                    // 如果没有旧数据
                    // 直接追加
                    data.buffer_size = data.buffer_size + msg.data.len() as u32;
                    data.game_data.push(msg.into());
                }
            }
        }

        pub fn on_client_begin_authentication(
            &mut self,
            auth: MsgClientBeginAuthentication,
            remote: SteamId,
        ) {
            if self
                .rg_client_data
                .iter()
                .find(|f| f.steam_iduser.steam_id().unwrap() == remote)
                .is_some()
            {
                return;
            }

            if self.rg_client_data.len() + self.rg_pending_client_data.len()
                >= self.max_players.into()
            {
                let mut br = false;
                self.rg_pending_client_data.retain_mut(|f| {
                    if f.steam_iduser.steam_id().unwrap() == remote && f.active {
                        f.hsteam_net_connection.take().unwrap().close(
                            NetConnectionEndReason::NetConnectionEnd(
                                networking_types::NetConnectionEnd::AppException,
                            ),
                            Some("Server full"),
                            false,
                        );

                        br = true;
                        return false;
                    }

                    return true;
                });

                if br {
                    return;
                }
            }

            self.rg_pending_client_data.retain_mut(|f| {
                if f.steam_iduser.steam_id().unwrap() == remote {
                    let res = self
                        .server_raw
                        .as_ref()
                        .unwrap()
                        .begin_authentication_session(remote, &auth.rgch_token);

                    if let Err(_) = res {
                        f.hsteam_net_connection.take().unwrap().close(
                            NetConnectionEndReason::NetConnectionEnd(
                                networking_types::NetConnectionEnd::AppException,
                            ),
                            Some("BeginAuthSession failed"),
                            false,
                        );

                        return false;
                    }

                    f.ul_tick_count_last_data = now();
                    f.active = true;
                }

                return true;
            });
        }

        pub fn remove_player_from_server(&mut self) {}

        pub fn on_auth_completed(
            &mut self,
            auth_successful: bool,
            pending_auth_index: usize,
        ) -> bool {
            if !self.rg_pending_client_data[pending_auth_index].active {
                println!("got auth completed callback for client who is not pending");
                return false;
            }

            if !auth_successful {
                self.server_raw
                    .as_ref()
                    .unwrap()
                    .end_authentication_session(
                        self.rg_pending_client_data[pending_auth_index]
                            .steam_iduser
                            .steam_id()
                            .unwrap(),
                    );

                self.send_message(
                    MsgServerFailAuthentication,
                    self.rg_pending_client_data[pending_auth_index]
                        .hsteam_net_connection
                        .as_ref()
                        .unwrap(),
                );
                return false;
            }

            let mut data = self.rg_pending_client_data.remove(pending_auth_index);
            data.ul_tick_count_last_data = now();

            self.send_message(
                MsgServerPassAuthentication {
                    player_position: pending_auth_index as u32,
                },
                data.hsteam_net_connection.as_ref().unwrap(),
            );
            self.rg_client_data.push(data);

            if self.rg_client_data.len() < self.max_players.into() {
                #[cfg(feature = "dev")]
                dbg!(self.rg_client_data.len() < self.max_players.into());

                return false;
            }

            if self.rg_client_data.iter().all(|f| f.active) {
                let ready = MsgServerAllReadyToGo;
                self.rg_client_data.iter().for_each(|f| {
                    self.send_message_ref(&ready, f.hsteam_net_connection.as_ref().unwrap());
                });

                return true;
            }

            false
        }

        pub fn send_message<T>(&self, msg: T, conn: &NetConnection<ServerManager>)
        where
            T: INetMessage + serde::Serialize,
        {
            //TODO u8 pool alloc and free
            self.send_message_ref(&msg, conn);
        }

        pub fn send_message_ref<T>(&self, msg: &T, conn: &NetConnection<ServerManager>)
        where
            T: INetMessage + serde::Serialize,
        {
            //TODO u8 pool alloc and free

            let mut bytes = Vec::new();
            msg.serialize(&mut rmps::Serializer::new(&mut bytes))
                .unwrap();

            let mut header: Vec<u8> = T::ID.into();
            header.append(&mut bytes);

            let mut message = self.utils.as_ref().unwrap().allocate_message(0);
            message.set_connection(conn);
            message.set_send_flags(SendFlags::RELIABLE_NO_NAGLE);
            message.set_data(header).unwrap();

            let results = self
                .listen_socket
                .as_ref()
                .unwrap()
                .send_messages(vec![message]);

            results.iter().for_each(|result| {
                let _ = result
                    .map_err(|e| match e {
                        SteamError::InvalidParameter => println!("SteamServerFailed sending data to server: Invalid connection handle, or the individual message is too big"),
                        SteamError::InvalidState => println!("SteamServerFailed sending data to server: Connection is in an invalid state"),
                        SteamError::NoConnection => println!("SteamServerFailed sending data to server: Connection has ended"),
                        SteamError::LimitExceeded => println!("SteamServerFailed sending data to server: There was already too much data queued to be sent"),
                        _ => println!("SteamServerSendMessageToConnection error,{:?}",e as i32)
                    });
            });
        }

        pub fn register(&mut self) {
            if let Some(svr) = self.server_raw.as_ref() {
                #[cfg(feature = "dev")]
                dbg!("JsSteamServer Handle register");

                let steam_servers_connected_send = self.send.as_mut().unwrap().clone();
                let steam_server_connect_failure_send = self.send.as_mut().unwrap().clone();
                let steam_servers_disconnected_send = self.send.as_mut().unwrap().clone();
                let validate_auth_ticket_response_send = self.send.as_mut().unwrap().clone();
                let gspolicy_response_callback_send = self.send.as_mut().unwrap().clone();

                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.register_callback(move |_: SteamServersConnected| {
                        #[cfg(feature = "dev")]
                        dbg!("SteamServersConnected Event");

                        steam_servers_connected_send
                            .send(SteamServerEvent::SteamServersConnected(
                                SteamServersConnected,
                            ))
                            .unwrap();
                    })),
                });

                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.register_callback(
                        move |v: steamworks::SteamServerConnectFailure| {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServerConnectFailure Event");

                            steam_server_connect_failure_send
                                .send(SteamServerEvent::SteamServerConnectFailure(
                                    SteamServerConnectFailure {
                                        reason: v.reason as i64,
                                        still_retrying: v.still_retrying,
                                    },
                                ))
                                .unwrap();
                        },
                    )),
                });

                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.register_callback(
                        move |v: steamworks::SteamServersDisconnected| {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServersDisconnected Event");

                            steam_servers_disconnected_send
                                .send(SteamServerEvent::SteamServersDisconnected(
                                    SteamServersDisconnected {
                                        reason: v.reason as i64,
                                    },
                                ))
                                .unwrap();
                        },
                    )),
                });

                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.register_callback(
                        move |v: steamworks::ValidateAuthTicketResponse| {
                            #[cfg(feature = "dev")]
                            dbg!("ValidateAuthTicketResponse Event");

                            validate_auth_ticket_response_send
                                .send(SteamServerEvent::ValidateAuthTicketResponse(
                                    ValidateAuthTicketResponse {
                                        steam_id: BigInt::from(v.steam_id.raw()),
                                        response: v.response.err().map(|e| e as i32),
                                        owner_steam_id: BigInt::from(v.owner_steam_id.raw()),
                                    },
                                ))
                                .unwrap();
                        },
                    )),
                });

                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.register_callback(
                        move |v: networking_utils::GSPolicyResponseCallback| {
                            #[cfg(feature = "dev")]
                            dbg!("GSPolicyResponseCallback Event");

                            gspolicy_response_callback_send
                                .send(SteamServerEvent::GSPolicyResponseCallback(
                                    GSPolicyResponseCallback { secure: v.secure },
                                ))
                                .unwrap();
                        },
                    )),
                });

                #[cfg(feature = "dev")]
                self.handle.as_mut().unwrap().insert(Handle {
                    handle: Some(svr.networking_utils().relay_network_status_callback(
                        move |v: networking_utils::RelayNetworkStatus| {
                            dbg!(
                                "JSSteamServer RelayNetworkStatus Event",
                                v.debugging_message()
                            );
                        },
                    )),
                });
            }
        }
    }
}
