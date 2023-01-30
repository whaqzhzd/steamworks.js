use napi_derive::napi;

#[napi]
pub mod steamp2p {
    use crate::api::callback::callback::Handle;
    use crate::api::p2p::message::*;
    use crate::client::now;
    use bytebuffer::ByteBuffer;
    use bytebuffer::Endian;
    use napi::bindgen_prelude::BigInt;
    use napi::bindgen_prelude::Buffer;
    use napi::threadsafe_function::ErrorStrategy;
    use napi::threadsafe_function::ThreadsafeFunction;
    use napi::threadsafe_function::ThreadsafeFunctionCallMode;
    use napi::JsFunction;
    use std::collections::HashSet;
    use std::sync::mpsc::channel;
    use std::sync::mpsc::Receiver;
    use std::sync::mpsc::Sender;
    use steamworks::networking_sockets::NetworkingSockets;
    use steamworks::networking_types;
    use steamworks::networking_types::NetConnectionEndReason;
    use steamworks::networking_types::NetworkingIdentity;
    use steamworks::networking_types::SendFlags;
    use steamworks::AuthTicket;
    use steamworks::Client;
    use steamworks::DurationControlOnlineState;
    use steamworks::LobbyGameCreated;
    use steamworks::LobbyId;
    use steamworks::Matchmaking;
    use steamworks::SteamError;
    use steamworks::SteamId;
    use steamworks::User;
    use steamworks::{
        networking_sockets::NetConnection, networking_utils::NetworkingUtils, ClientManager,
    };

    #[derive(PartialEq, Eq)]
    enum SteamClientState {
        KEclientFree,
        KEclientInLobby,
        KEclientGameConnecting,
    }

    // 客户端连接状态
    #[derive(PartialEq, Eq)]
    enum EClientConnectionState {
        KEclientNotConnected,                   // 初始状态，未连接到服务器
        KEclientConnectedPendingAuthentication, // 我们已经与服务器建立了通信，但它还没有授权给我们。
        KEclientConnectedAndAuthenticated, // 最后阶段，服务器已经授权给我们，我们实际上可以在上面玩了
    }

    enum SteamClientEvent {
        LobbyGameCreated(LobbyGameCreated),
    }

    #[napi]
    pub struct SteamReceiveUpdate {
        pub buffer: Buffer,
        pub frame_id: u32,
        pub count: u32,
    }

    #[napi]
    pub struct GameStart {
        pub buffer: Buffer,
        pub count: u32,
    }

    #[napi]
    pub struct BroadcastData {
        pub buffer: Buffer,
        pub steam_id: BigInt,
    }

    #[napi]
    pub struct SteamClientManager {
        rx: Receiver<SteamClientEvent>,
        raw: JsSteamClient,
    }

    #[napi]
    impl SteamClientManager {
        pub fn receive(&mut self) {
            let mut client = &mut self.raw;

            loop {
                if let Some(conn) = client.conn_server.as_ref() {
                    let messages = conn.receive_messages(32);
                    for message in messages {
                        #[cfg(feature = "dev")]
                        dbg!("SteamClientManager::receive message");

                        client.last_network_data_received_time = now();

                        let data = message.data();

                        //  确保网络已经联通
                        if client.connected_status == EClientConnectionState::KEclientNotConnected
                            && client.state != SteamClientState::KEclientGameConnecting
                        {
                            #[cfg(feature = "dev")]
                            dbg!(
                                client.connected_status
                                    == EClientConnectionState::KEclientNotConnected
                                    && client.state != SteamClientState::KEclientGameConnecting
                            );

                            drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                            continue;
                        }

                        if data.len() < 4 {
                            println!("got garbage on client socket, too short");

                            drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                            continue;
                        }

                        let header: EMessage = data[0..4].to_vec().into();
                        let body = &data[4..];

                        if header == EMessage::Error {
                            #[cfg(feature = "dev")]
                            dbg!("SteamClientManager::receive EMessage::Error");

                            drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                            continue;
                        }

                        #[cfg(feature = "dev")]
                        dbg!(header);

                        match header {
                            EMessage::KEmsgServerSendInfo => {
                                if let Ok(msg) = rmps::from_slice::<MsgServerSendInfo>(body) {
                                    client.on_receive_server_info(msg);
                                }
                            }
                            EMessage::KEmsgServerFailAuthentication => {
                                if let Ok(_) = rmps::from_slice::<MsgServerFailAuthentication>(body)
                                {
                                    client.on_receive_server_authentication_response(false, 0);
                                }
                            }
                            EMessage::KEmsgServerPassAuthentication => {
                                if let Ok(msg) =
                                    rmps::from_slice::<MsgServerPassAuthentication>(body)
                                {
                                    client.on_receive_server_authentication_response(
                                        true,
                                        msg.player_position,
                                    );
                                }
                            }
                            EMessage::KEmsgServerAllReadyToGo => {
                                if let Some(fun) = client.steam_all_ready_to_go.as_ref() {
                                    fun.call((), ThreadsafeFunctionCallMode::Blocking);
                                }
                            }
                            EMessage::KEmsgServerFramesData => {
                                if let Ok(msg) = rmps::from_slice::<MsgServerFramesData>(body) {
                                    client.on_receive_update(msg);
                                }
                            }
                            EMessage::KEmsgServerGameStart => {
                                if let Ok(msg) = rmps::from_slice::<MsgServerGameStart>(body) {
                                    client.on_game_start(msg);
                                }
                            }
                            EMessage::KEmsgServerSetGameStartDataComplete => {
                                if let Some(fun) = client.set_game_start_data.as_ref() {
                                    fun.call((), ThreadsafeFunctionCallMode::Blocking);
                                }
                            }
                            EMessage::KEmsgServerBroadcast => {
                                if let Ok(msg) = rmps::from_slice::<MsgServerDataBroadcast>(body) {
                                    client.on_broadcast_update(msg);
                                }
                            }
                            _ => panic!("error message,{:?}", header),
                        }

                        drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                    }

                    if let Ok(result) = self.rx.try_recv() {
                        match result {
                            SteamClientEvent::LobbyGameCreated(created) => {
                                #[cfg(feature = "dev")]
                                dbg!("SteamClientEvent::LobbyGameCreated");

                                #[cfg(feature = "dev")]
                                dbg!(client.state != SteamClientState::KEclientInLobby);

                                if client.state != SteamClientState::KEclientInLobby {
                                    return;
                                }

                                client.initiate_server_connection(BigInt::from(
                                    created.ul_steam_idgame_server,
                                ));
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        pub fn on_net_connection_status_changed(&mut self) {}

        #[napi]
        pub fn initialize(&mut self) {
            self.raw.initialize();
        }

        #[napi(ts_args_type = "callback: (count:number) => void")]
        pub fn on_steam_connected_success(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<u32, ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();
            self.raw.steam_connected_success = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_steam_connected_success");
        }

        #[napi(ts_args_type = "callback: () => void")]
        pub fn on_steam_all_ready_to_go(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<(), ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();
            self.raw.steam_all_ready_to_go = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_steam_all_ready_to_go");
        }

        #[napi(
            ts_args_type = "callback: ({buffer,frameID,count}:{buffer:ArrayBuffer,frameID:number,count:number}) => void"
        )]
        pub fn on_steam_on_receive_update(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<SteamReceiveUpdate, ErrorStrategy::Fatal> =
                handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();
            self.raw.steam_on_receive_update = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("on_steam_on_receive_update");
        }

        #[napi(
            ts_args_type = "callback: ({buffer,count}:{buffer:ArrayBuffer,count:number}) => void"
        )]
        pub fn game_start_data_callback(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<GameStart, ErrorStrategy::Fatal> = handler
                .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                .unwrap();
            self.raw.game_start_data_cb = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("game_start_data_callback");
        }

        #[napi(
            ts_args_type = "callback: ({buffer,steamID}:{buffer:ArrayBuffer,steamID:bigint}) => void"
        )]
        pub fn broadcast_callback(&mut self, handler: JsFunction) {
            let threadsafe_handler: ThreadsafeFunction<BroadcastData, ErrorStrategy::Fatal> =
                handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();
            self.raw.broadcast_cb = Some(threadsafe_handler);

            #[cfg(feature = "dev")]
            dbg!("broadcast_callback");
        }

        #[napi]
        pub fn is_connected_to_server(&self) -> bool {
            self.raw.connected_status == EClientConnectionState::KEclientConnectedAndAuthenticated
        }

        #[napi]
        pub fn load_ready_to_go(&self) {
            self.raw.send_message(MsgClientLoadComplete);
        }

        #[napi]
        pub fn run_callback(
            &mut self,
            is_connected_to_server: bool,
            policy_response_callback: bool,
        ) {
            if !self.raw.checkout {
                return;
            }

            match self.raw.state {
                SteamClientState::KEclientFree => {}
                SteamClientState::KEclientInLobby => {
                    if is_connected_to_server && policy_response_callback {
                        #[cfg(feature = "dev")]
                        dbg!(
                            "JsSteamClient set_lobby_game_server",
                            self.raw.steam_id_game_server.as_ref().unwrap().raw(),
                            self.raw.lobby_id.as_ref().unwrap().raw()
                        );

                        self.raw.matchmaking.set_lobby_game_server(
                            LobbyId::from_raw(self.raw.lobby_id.as_ref().unwrap().raw()),
                            0,
                            0,
                            self.raw.steam_id_game_server.as_ref().unwrap().raw(),
                        );

                        self.raw.initiate_server_connection(BigInt::from(
                            self.raw.steam_id_game_server.as_ref().unwrap().raw(),
                        ));
                    }
                }
                SteamClientState::KEclientGameConnecting => {}
            }

            self.receive();
        }

        #[napi]
        pub fn set_lobby_id(&mut self, lobby_id: BigInt) {
            self.raw.lobby_id = Some(SteamId::from_raw(lobby_id.get_u64().1));

            #[cfg(feature = "dev")]
            dbg!("set_lobby_id", self.raw.lobby_id);
        }

        #[napi]
        pub fn set_steam_id_game_server(&mut self, lobby_id: BigInt) {
            self.raw.steam_id_game_server = Some(SteamId::from_raw(lobby_id.get_u64().1));

            #[cfg(feature = "dev")]
            dbg!("set_steam_id_game_server", self.raw.steam_id_game_server);
        }

        #[napi]
        pub fn send_frame_data(&self, types: u32, buffer: Buffer) {
            if buffer.len() == 0 {
                return;
            }

            if types == 0 {
                return;
            }

            self.raw.send_message(MsgClientFrameData {
                data: buffer.to_vec(),
                types,
            });
        }

        #[napi]
        pub fn set_game_data(&self, buffer: Buffer) {
            if buffer.len() == 0 {
                return;
            }

            self.raw.send_message(MsgClientFrameData {
                data: buffer.to_vec(),
                types: 0,
            });
        }

        #[napi]
        pub fn broadcast(&self, buffer: Buffer) {
            if buffer.len() == 0 {
                return;
            }

            self.raw.send_message(MsgClientDataBroadcast {
                data: buffer.to_vec(),
                types: 0,
                local_steam_id: 0,
            });
        }
    }

    #[napi]
    pub fn create_async_client() -> SteamClientManager {
        let mut client = JsSteamClient::new();
        let (tx, rx) = channel();

        client.send = Some(tx);
        client.handle = Some(HashSet::new());

        SteamClientManager { rx, raw: client }
    }

    #[napi(js_name = "SteamClient")]
    pub struct JsSteamClient {
        // 权限ticket
        auth_ticket: Option<AuthTicket>,

        /// 服务器ip
        un_server_ip: u32,

        /// 服务器端口
        us_server_port: u16,

        // 玩家在服务器中的槽位
        player_index: u32,
        state: SteamClientState,
        connected_status: EClientConnectionState,
        conn_server: Option<NetConnection<ClientManager>>,
        checkout: bool,
        local_id: Option<SteamId>,
        lobby_id: Option<SteamId>,
        utils: Option<NetworkingUtils<ClientManager>>,
        handle: Option<HashSet<Handle>>,
        send: Option<Sender<SteamClientEvent>>,
        client_raw: Option<Client<ClientManager>>,
        client_socket: Option<NetworkingSockets<ClientManager>>,
        // 我们最后一次从服务器获得数据的时间
        last_network_data_received_time: i64,
        steam_id_game_server: Option<SteamId>,
        user: User<ClientManager>,
        matchmaking: Matchmaking<ClientManager>,

        steam_connected_success: Option<ThreadsafeFunction<u32, ErrorStrategy::Fatal>>,
        steam_all_ready_to_go: Option<ThreadsafeFunction<(), ErrorStrategy::Fatal>>,
        steam_on_receive_update:
            Option<ThreadsafeFunction<SteamReceiveUpdate, ErrorStrategy::Fatal>>,
        game_start_data_cb: Option<ThreadsafeFunction<GameStart, ErrorStrategy::Fatal>>,
        set_game_start_data: Option<ThreadsafeFunction<(), ErrorStrategy::Fatal>>,
        broadcast_cb: Option<ThreadsafeFunction<BroadcastData, ErrorStrategy::Fatal>>,
    }

    #[napi]
    impl JsSteamClient {
        #[napi(constructor)]
        pub fn new() -> Self {
            let client = crate::client::get_client();

            JsSteamClient {
                auth_ticket: None,
                un_server_ip: 0,
                us_server_port: 0,
                player_index: 0,
                state: SteamClientState::KEclientFree,
                connected_status: EClientConnectionState::KEclientNotConnected,
                conn_server: None,
                checkout: false,
                local_id: None,
                lobby_id: None,
                utils: None,
                handle: None,
                send: None,
                client_raw: None,
                client_socket: None,
                last_network_data_received_time: 0,
                steam_id_game_server: None,
                user: client.user(),
                matchmaking: client.matchmaking(),

                steam_connected_success: None,
                steam_all_ready_to_go: None,
                steam_on_receive_update: None,
                game_start_data_cb: None,
                broadcast_cb: None,
                set_game_start_data: None,
            }
        }

        #[napi]
        pub fn initialize(&mut self) {
            #[cfg(feature = "dev")]
            dbg!("JsSteamClient initialize");

            let client = crate::client::get_client();
            self.local_id = Some(self.user.steam_id());

            self.checkout = true;
            self.state = SteamClientState::KEclientInLobby;
            self.connected_status = EClientConnectionState::KEclientNotConnected;

            let socket = client.networking_sockets();
            let utils = client.networking_utils();

            self.client_raw = Some(client);
            self.utils = Some(utils);
            self.client_socket = Some(socket);

            self.register();
            self.init_relay_network_access();
        }

        #[napi]
        pub fn init_relay_network_access(&self) {
            self.utils.as_ref().unwrap().init_relay_network_access();
        }

        #[napi]
        pub fn initiate_server_connection(&mut self, server: BigInt) {
            #[cfg(feature = "dev")]
            dbg!("JsSteamClient initiate_server_connection");

            self.state = SteamClientState::KEclientGameConnecting;

            let identity = NetworkingIdentity::new_steam_id(SteamId::from_raw(server.get_u64().1));

            #[cfg(feature = "dev")]
            dbg!(identity.steam_id());

            let p2p = self
                .client_socket
                .as_ref()
                .unwrap()
                .connect_p2p(identity, 0, vec![]);

            if let Ok(p2p) = p2p {
                self.conn_server = Some(p2p);

                #[cfg(feature = "dev")]
                dbg!("JsSteamClient initiate_server_connection success");
            } else {
                self.conn_server = None;
            }

            //TODO 设置语音服务器的网络句柄

            self.last_network_data_received_time = now();
        }
    }

    #[napi]
    impl JsSteamClient {
        pub fn register(&mut self) {
            if let Some(client) = self.client_raw.as_ref() {
                #[cfg(feature = "dev")]
                dbg!("JsSteamClient Handle register");

                let steam_servers_connected_send = self.send.as_mut().unwrap().clone();

                self.handle
                    .as_mut()
                    .unwrap()
                    .insert(Handle::new(Some(client.register_callback(
                        move |created: LobbyGameCreated| {
                            #[cfg(feature = "dev")]
                            dbg!("LobbyGameCreated Event");

                            steam_servers_connected_send
                                .send(SteamClientEvent::LobbyGameCreated(created))
                                .unwrap();
                        },
                    ))));
            }
        }

        pub fn on_broadcast_update(&mut self, data: MsgServerDataBroadcast) {
            let mut buffer = ByteBuffer::new();
            buffer.set_endian(Endian::LittleEndian);

            let count = data.data.len();

            if count != 0 {
                buffer.write_bytes(&data.data);
            }

            if let Some(fun) = self.broadcast_cb.as_ref() {
                fun.call(
                    BroadcastData {
                        buffer: Buffer::from(buffer.into_vec()),
                        steam_id: BigInt::from(data.local_steam_id),
                    },
                    ThreadsafeFunctionCallMode::Blocking,
                );
            }
        }

        pub fn on_game_start(&mut self, data: MsgServerGameStart) {
            let mut buffer = ByteBuffer::new();
            buffer.set_endian(Endian::LittleEndian);

            let mut count = data.game_data.len();
            let u16size = std::mem::size_of::<u16>();
            let size = data.buffer_size as usize + u16size * count;
            buffer.resize(size);

            if count != 0 {
                let mut offset = 0;
                let frame_data = &data.game_data;

                for frame in frame_data.iter() {
                    buffer.write_u16(frame.data.len() as u16);
                    offset += u16size;
                    buffer.write_bytes(&frame.data);
                    offset += frame.data.len();
                }

                if offset != size {
                    buffer.clear();
                    buffer.resize(0);
                    count = 0;
                }
            }

            if let Some(fun) = self.game_start_data_cb.as_ref() {
                fun.call(
                    GameStart {
                        buffer: Buffer::from(buffer.into_vec()),
                        count: count.try_into().unwrap(),
                    },
                    ThreadsafeFunctionCallMode::Blocking,
                );
            }
        }

        pub fn on_receive_update(&mut self, data: MsgServerFramesData) {
            let mut buffer = ByteBuffer::new();
            buffer.set_endian(Endian::LittleEndian);

            let mut count = data.game_data.len();
            let u16size = std::mem::size_of::<u16>();
            let size = data.buffer_size as usize + u16size * count;
            buffer.resize(size);

            if count != 0 {
                let mut offset = 0;
                let frame_data = &data.game_data;

                for frame in frame_data.iter() {
                    buffer.write_u16(frame.data.len() as u16);
                    offset += u16size;
                    buffer.write_bytes(&frame.data);
                    offset += frame.data.len();
                }

                if offset != size {
                    buffer.clear();
                    buffer.resize(0);
                    count = 0;
                }
            }

            if let Some(fun) = self.steam_on_receive_update.as_ref() {
                fun.call(
                    SteamReceiveUpdate {
                        buffer: Buffer::from(buffer.into_vec()),
                        frame_id: data.frame_id,
                        count: count.try_into().unwrap(),
                    },
                    ThreadsafeFunctionCallMode::Blocking,
                );
            }
        }

        pub fn on_receive_server_authentication_response(&mut self, success: bool, pos: u32) {
            if !success {
                self.disconnect_from_server();
            } else {
                if self.connected_status
                    == EClientConnectionState::KEclientConnectedAndAuthenticated
                    && self.player_index == pos
                {
                    return;
                }

                self.player_index = pos;
                self.connected_status = EClientConnectionState::KEclientConnectedAndAuthenticated;
                self.user.set_duration_control_online_state(
                    DurationControlOnlineState::DurationControlOnlineStateOnlineHighPri,
                );

                if let Some(fun) = self.steam_connected_success.as_ref() {
                    fun.call(1, ThreadsafeFunctionCallMode::Blocking);
                }
            }
        }

        pub fn disconnect_from_server(&mut self) {
            if self.connected_status != EClientConnectionState::KEclientNotConnected {
                if self.auth_ticket.is_some() {
                    self.user
                        .cancel_authentication_ticket(self.auth_ticket.take().unwrap());
                }

                // tell steam china duration control system that we are no longer in a match
                self.user.set_duration_control_online_state(
                    DurationControlOnlineState::DurationControlOnlineStateOffline,
                );
                self.connected_status = EClientConnectionState::KEclientNotConnected;
            }

            //TODO stop voice chat

            if self.conn_server.is_some() {
                self.conn_server.take().unwrap().close(
                    NetConnectionEndReason::NetConnectionEnd(
                        networking_types::NetConnectionEnd::AppException,
                    ),
                    None,
                    false,
                );
            }

            self.steam_id_game_server = None;
        }

        /// extract the IP address of the user from the socket
        pub fn on_receive_server_info(&mut self, msg: MsgServerSendInfo) {
            #[cfg(feature = "dev")]
            dbg!("JsSteamClient on_receive_server_info");

            self.connected_status = EClientConnectionState::KEclientConnectedPendingAuthentication;
            self.steam_id_game_server = Some(SteamId::from_raw(msg.ul_steam_idserver));

            if let Some(info) = self.conn_server.as_ref().unwrap().get_connection_info() {
                self.un_server_ip = info.ip_v4().map_or(0, |f| f.into());
                self.us_server_port = info.port();
            }

            let (auth_ticket, ticket) = self
                .client_raw
                .as_ref()
                .unwrap()
                .user()
                .authentication_session_ticket();

            if ticket.len() < 1 {
                println!("Warning: Looks like GetAuthSessionTicket didn't give us a good ticket");
            }

            self.auth_ticket = Some(auth_ticket);

            let auth = MsgClientBeginAuthentication { rgch_token: ticket };
            self.send_message(auth);
        }

        pub fn send_message<T>(&self, msg: T)
        where
            T: INetMessage + serde::Serialize,
        {
            //TODO u8 pool alloc and free
            let mut bytes = Vec::new();
            msg.serialize(&mut rmps::Serializer::new(&mut bytes))
                .unwrap();

            let mut header: Vec<u8> = T::ID.into();
            header.append(&mut bytes);

            let result = self
                .conn_server
                .as_ref()
                .unwrap()
                .send_message(&header, SendFlags::RELIABLE_NO_NAGLE);

            let _ = result
                .map_err(|e| match e {
                    SteamError::InvalidParameter => println!("SteamClientFailed sending data to server: Invalid connection handle, or the individual message is too big"),
                    SteamError::InvalidState => println!("SteamClientFailed sending data to server: Connection is in an invalid state"),
                    SteamError::NoConnection => println!("SteamClientFailed sending data to server: Connection has ended"),
                    SteamError::LimitExceeded => println!("SteamClientFailed sending data to server: There was already too much data queued to be sent"),
                    _ => println!("SendMessageToConnection error,{:?}",e as i32)
                });
        }
    }
}
