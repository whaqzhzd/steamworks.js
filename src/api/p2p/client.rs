use napi_derive::napi;

#[napi]
pub mod steamp2p {
    use crate::api::callback::callback::Handle;
    use crate::api::p2p::message::*;
    use crate::client::now;
    use napi::bindgen_prelude::BigInt;
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
    use steamworks::networking_types::NetworkingIdentity;
    use steamworks::networking_types::SendFlags;
    use steamworks::AuthTicket;
    use steamworks::Client;
    use steamworks::DurationControlOnlineState;
    use steamworks::LobbyGameCreated;
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
    pub struct SteamClientManager {
        rx: Receiver<SteamClientEvent>,
        raw: JsSteamClient,
    }

    #[napi]
    impl SteamClientManager {
        pub fn receive(&mut self) {
            let mut client = &mut self.raw;

            loop {
                let messages = client.conn_server.as_ref().unwrap().receive_messages(32);
                for message in messages {
                    client.last_network_data_received_time = now();

                    let data = message.data();

                    //  确保网络已经联通
                    if client.connected_status == EClientConnectionState::KEclientNotConnected
                        && client.state != SteamClientState::KEclientGameConnecting
                    {
                        drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                        continue;
                    }

                    if data.len() < 4 {
                        println!("got garbage on client socket, too short");

                        drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                        continue;
                    }

                    let header: EMessage = data[0..4].to_vec().into();
                    let body = &data[5..];

                    if header == EMessage::Error {
                        drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                        continue;
                    }

                    match header {
                        EMessage::KEmsgServerSendInfo => {
                            if let Ok(msg) = rmps::from_slice::<MsgServerSendInfo>(body) {
                                client.on_receive_server_info(msg);
                            }
                        }
                        EMessage::KEmsgServerFailAuthentication => {
                            if let Ok(_) = rmps::from_slice::<MsgServerFailAuthentication>(body) {
                                client.on_receive_server_authentication_response(false, 0);
                            }
                        }
                        EMessage::KEmsgServerPassAuthentication => todo!(),
                        EMessage::KEmsgServerAllReadyToGo => todo!(),
                        EMessage::KEmsgServerFramesData => todo!(),
                        EMessage::KEmsgServerGameStart => todo!(),
                        EMessage::KEmsgServerSetGameStartDataComplete => todo!(),
                        EMessage::KEmsgServerBroadcast => todo!(),
                        _ => panic!("error message,{:?}", header),
                    }

                    drop(message); // drop call SteamAPI_SteamNetworkingMessage_t_Release
                }

                if let Ok(result) = self.rx.try_recv() {
                    match result {
                        SteamClientEvent::LobbyGameCreated(created) => {
                            #[cfg(feature = "dev")]
                            dbg!("SteamClientEvent::LobbyGameCreated");

                            if client.state == SteamClientState::KEclientInLobby {
                                return;
                            }

                            if SteamId::from_raw(created.ul_steam_idgame_server).is_valid() {
                                client.initiate_server_connection(BigInt::from(
                                    created.ul_steam_idgame_server,
                                ));
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

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
        local_id: SteamId,
        utils: Option<NetworkingUtils<ClientManager>>,
        handle: Option<HashSet<Handle>>,
        send: Option<Sender<SteamClientEvent>>,
        client_raw: Option<Client<ClientManager>>,
        client_socket: Option<NetworkingSockets<ClientManager>>,
        // 我们最后一次从服务器获得数据的时间
        last_network_data_received_time: i64,
        steam_id_game_server: SteamId,
        user: User<ClientManager>,

        steam_connected_success: Option<ThreadsafeFunction<u32, ErrorStrategy::Fatal>>,
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
                local_id: SteamId::from_raw(0),
                utils: None,
                handle: None,
                send: None,
                client_raw: None,
                client_socket: None,
                last_network_data_received_time: 0,
                steam_id_game_server: SteamId::from_raw(0),
                user: client.user(),

                steam_connected_success: None,
            }
        }

        #[napi]
        pub fn initialize(&mut self) {
            let client = crate::client::get_client();
            self.local_id = self.user.steam_id();

            self.checkout = true;
            self.state = SteamClientState::KEclientInLobby;
            self.connected_status = EClientConnectionState::KEclientNotConnected;

            let socket = client.networking_sockets();
            let utils = client.networking_utils();

            self.client_raw = Some(client);
            self.utils = Some(utils);
            self.client_socket = Some(socket);
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

            let p2p = self
                .client_socket
                .as_ref()
                .unwrap()
                .connect_p2p(identity, 0, vec![]);

            if let Ok(p2p) = p2p {
                self.conn_server = Some(p2p);
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
                    networking_types::NetConnectionEnd::AppException,
                    None,
                    false,
                );
            }
            self.steam_id_game_server = SteamId::from_raw(0);
        }

        /// extract the IP address of the user from the socket
        pub fn on_receive_server_info(&mut self, msg: MsgServerSendInfo) {
            #[cfg(feature = "dev")]
            dbg!("JsSteamClient Handle register");

            self.connected_status = EClientConnectionState::KEclientConnectedPendingAuthentication;
            self.steam_id_game_server = SteamId::from_raw(msg.ul_steam_idserver);

            if let Some(info) = self.conn_server.as_ref().unwrap().get_connection_info() {
                self.un_server_ip = info.ip_v4();
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
