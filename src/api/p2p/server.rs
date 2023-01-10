use napi_derive::napi;

#[napi]
pub mod steamp2p {
    use napi::bindgen_prelude::ToNapiValue;
    use napi::bindgen_prelude::*;
    use napi::{
        threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
        JsFunction,
    };
    use networking_sockets::*;
    use std::collections::HashSet;
    use std::hash::Hash;
    use std::hash::Hasher;
    use std::net::Ipv4Addr;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use steamworks::networking_types::ListenSocketEvent;
    use steamworks::{SteamServersConnected, *};

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
        steam_iduser: SteamId,
        ul_tick_count_last_data: u64,
        hsteam_net_connection: u32,
    }

    impl ClientConnectionData {
        pub fn new() -> Self {
            ClientConnectionData {
                active: false,
                load_complete: false,
                ul_tick_count_last_data: 0,
                hsteam_net_connection: 0,
                steam_iduser: SteamId::from_raw(0),
            }
        }
    }

    #[napi(js_name = "SteamServer")]
    pub struct JsSteamServer {
        is_connected_to_steam: bool,
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

        listen_socket: Option<ListenSocket<ClientManager>>, // drop CloseListenSocket
        net_poll_group: Option<NetPollGroup<ClientManager>>, // drop DestroyPollGroup
        server_raw: Option<Server>,                         // drop LogOff Shutdown
        server_single: Option<SingleClient<ServerManager>>,
        server_sockets: Option<NetworkingSockets<ClientManager>>,

        handle: Option<HashSet<Handle>>,

        send: Option<Sender<SteamServerEvent>>,
    }

    #[napi]
    pub struct SteamServerManager {
        rx: Receiver<SteamServerEvent>,
        tx: Sender<SteamServerEvent>,
        raw: Arc<Mutex<JsSteamServer>>,

        steam_servers_connected: Option<ThreadsafeFunction<(), ErrorStrategy::Fatal>>,
    }

    #[napi]
    impl SteamServerManager {
        #[napi(ts_args_type = "callback: () => void")]
        pub fn on_servers_connected(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<(), ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();

            self.steam_servers_connected = Some(threadsafe_handler);
        }

        pub fn receive(&mut self) {
            let mut server = self.raw.lock().unwrap();

            loop {
                if let Some(socket) = server.listen_socket.as_ref() {
                    if let Some(event) = socket.try_receive_event() {
                        match event {
                            ListenSocketEvent::Connecting(request) => {
                                #[cfg(feature = "dev")]
                                dbg!("ListenSocketEvent::Connecting");

                                request.accept().unwrap();
                            }
                            _ => panic!("unexpected event"),
                        }
                    }
                }

                if let Ok(result) = self.rx.try_recv() {
                    match result {
                        SteamServerEvent::SteamServersConnected(_) => {
                            #[cfg(feature = "dev")]
                            dbg!("SteamServerEvent::SteamServersConnected");

                            server.is_connected_to_steam = true;
                            server.connected_success_count += 1;
                            server.send_updated_server_details_to_steam();

                            if let Some(fun) = self.steam_servers_connected.as_ref() {
                                fun.call((), ThreadsafeFunctionCallMode::Blocking);
                            }
                        }
                        SteamServerEvent::SteamServerConnectFailure(failure) => todo!(),
                        SteamServerEvent::SteamServersDisconnected(disconnected) => todo!(),
                        SteamServerEvent::ValidateAuthTicketResponse(response) => todo!(),
                        SteamServerEvent::GSPolicyResponseCallback(response) => todo!(),
                        _ => panic!(),
                    }
                } else {
                    break;
                }
            }
        }

        #[napi]
        pub fn run_callbacks(&mut self) {
            {
                self.raw.lock().unwrap().run_callbacks();
            }

            self.receive();
        }

        #[napi]
        pub fn is_connected_to_steam(&self) -> bool {
            self.raw.lock().unwrap().is_connected_to_steam
        }

        /// 设置应用ID
        #[napi]
        pub fn set_appid(&mut self, appid: u32) {
            self.raw.lock().unwrap().set_appid(appid);
        }

        /// 可以加入一个服务器并同时游戏的最大玩家数量
        #[napi]
        pub fn set_max_player(&mut self, max: u8) {
            self.raw.lock().unwrap().set_max_player(max);
        }

        /// 设置应用名称
        #[napi]
        pub fn set_app_name(&mut self, name: String) {
            self.raw.lock().unwrap().set_app_name(name);
        }

        /// 设置地图名称
        #[napi]
        pub fn set_map_name(&mut self, name: String) {
            self.raw.lock().unwrap().set_map_name(name);
        }

        /// 设置服务器名称
        #[napi]
        pub fn set_server_name(&mut self, name: String) {
            self.raw.lock().unwrap().set_server_name(name);
        }

        /// 设置机器人的数量
        #[napi]
        pub fn set_bot_player_count(&mut self, bot: u32) {
            self.raw.lock().unwrap().set_bot_player_count(bot);
        }

        /// 设置FPS
        #[napi]
        pub fn set_interval(&mut self, interval: f64) {
            self.raw.lock().unwrap().set_interval(interval);
        }

        /// 获取游戏服务器的steam 唯一ID
        #[napi]
        pub fn get_server_steam_id(&self) -> u64 {
            self.raw.lock().unwrap().get_server_steam_id()
        }

        /// 设置当前服务器的大厅唯一ID
        #[napi]
        pub fn set_lobby_id(&mut self, lobby_id: BigInt) {
            self.raw.lock().unwrap().set_lobby_id(lobby_id);
        }

        ///  获取大厅唯一ID
        #[napi]
        pub fn get_lobby_id(&self) -> u64 {
            self.raw.lock().unwrap().lobby_id
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
            self.raw.lock().unwrap().initialize(
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
            self.tx.send(SteamServerEvent::SteamServerOpen).unwrap();
        }
    }

    enum SteamServerEvent {
        SteamServersConnected(SteamServersConnected),
        SteamServerConnectFailure(SteamServerConnectFailure),
        SteamServersDisconnected(SteamServersDisconnected),
        ValidateAuthTicketResponse(ValidateAuthTicketResponse),
        GSPolicyResponseCallback(GSPolicyResponseCallback),

        SteamServerOpen,
    }

    #[napi]
    pub async fn create_async_server() -> SteamServerManager {
        let server = Arc::new(Mutex::new(JsSteamServer::new()));
        let (tx, rx) = channel();
        let (qtx, qrx) = channel();

        #[cfg(feature = "dev")]
        dbg!("create_async_server");

        let clone = server.clone();
        tokio::spawn(async move {
            #[cfg(feature = "dev")]
            dbg!("create_async_server tokio::spawn::entry");

            server.lock().unwrap().handle = Some(HashSet::new());
            server.lock().unwrap().send = Some(tx);

            for x in qrx {
                match x {
                    SteamServerEvent::SteamServerOpen => {
                        #[cfg(feature = "dev")]
                        dbg!("SteamServerOpen open");

                        server.lock().unwrap().open();
                    }
                    _ => panic!(),
                };
            }
        });

        SteamServerManager {
            rx,
            raw: clone,
            tx: qtx,
            steam_servers_connected: None,
        }
    }

    #[napi]
    impl JsSteamServer {
        #[napi(constructor)]
        pub fn new() -> Self {
            let server = JsSteamServer {
                is_connected_to_steam: false,
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

                handle: None,
                send: None,
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
                let client = crate::client::get_client();
                self.server_raw = Some(server);
                self.server_single = Some(single);
                self.server_sockets = Some(client.networking_server_sockets());

                #[cfg(feature = "dev")]
                dbg!("steam server init success");

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

                let client = crate::client::get_client();
                client.networking_utils().init_relay_network_access();

                if self.server_mode == EServerMode::EServerModeAuthenticationAndSecure {
                    server.enable_heartbeats(true);
                }

                #[cfg(feature = "dev")]
                dbg!("steam server init_relay_network_access success");
            }

            self.player_count = 0;
            self.game_state = EServerGameState::KEserverWaitingForPlayers;

            if let Some(sockets) = self.server_sockets.as_ref() {
                if let Ok(listen) = sockets.create_listen_socket_p2p(0, vec![]) {
                    self.listen_socket = Some(listen);

                    #[cfg(feature = "dev")]
                    dbg!("steam server create_listen_socket_p2p success");
                } else {
                    return;
                }
            } else {
                return;
            }

            if let Some(sockets) = self.server_sockets.as_ref() {
                self.net_poll_group = Some(sockets.create_poll_group());

                #[cfg(feature = "dev")]
                dbg!("steam server create_poll_group success");
            } else {
                return;
            }
            self.can_close = true;
        }

        pub fn register(&mut self) {
            if let Some(svr) = self.server_raw.as_ref() {
                #[cfg(feature = "dev")]
                dbg!("Handle register");

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
            }
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
}
