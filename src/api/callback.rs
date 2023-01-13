use napi_derive::napi;

#[napi]
pub mod callback {
    use napi::bindgen_prelude::BigInt;
    use napi::{
        bindgen_prelude::ToNapiValue,
        threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode},
        JsFunction,
    };
    use std::hash::Hash;
    use std::hash::Hasher;

    #[napi]
    pub struct Handle {
        handle: Option<steamworks::CallbackHandle>,
    }

    #[napi]
    impl Handle {
        pub fn new(handle: Option<steamworks::CallbackHandle>) -> Handle {
            Handle { handle }
        }

        #[napi]
        pub fn disconnect(&mut self) {
            self.handle = None;
        }
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
    pub enum SteamCallback {
        PersonaStateChange,
        SteamServersConnected,
        SteamServersDisconnected,
        SteamServerConnectFailure,
        LobbyDataUpdate,
        LobbyChatUpdate,
        LobbyChatMessage,
        P2PSessionRequest,
        P2PSessionConnectFail,
        RelayNetworkStatusCallback,
    }

    #[napi]
    pub enum PersonaChange {
        NAME = 0x0001,
        STATUS = 0x0002,
        ComeOnline = 0x0004,
        GoneOffline = 0x0008,
        GamePlayed = 0x0010,
        GameServer = 0x0020,
        AVATAR = 0x0040,
        JoinedSource = 0x0080,
        LeftSource = 0x0100,
        RelationshipChange = 0x0200,
        NameFirstSet = 0x0400,
        FacebookInfo = 0x0800,
        NICKNAME = 0x1000,
        SteamLevel = 0x2000,
    }

    #[napi]
    pub enum ChatMemberStateChange {
        /// This user has joined or is joining the lobby.
        Entered,

        /// This user has left or is leaving the lobby.
        Left,

        /// User disconnected without leaving the lobby first.
        Disconnected,

        /// The user has been kicked.
        Kicked,

        /// The user has been kicked and banned.
        Banned,
    }

    #[napi]
    pub struct PersonaStateChange {
        pub steam_id: BigInt,
        pub flags: i32,
    }

    #[napi]
    pub struct LobbyDataUpdate {
        pub lobby: BigInt,
        pub member: BigInt,
        pub success: bool,
    }

    #[napi]
    pub struct LobbyChatUpdate {
        /// The Steam ID of the lobby.
        pub lobby: BigInt,
        /// The user who's status in the lobby just changed - can be recipient.
        pub user_changed: BigInt,
        /// Chat member who made the change. This can be different from `user_changed` if kicking, muting, etc. For example, if one user kicks another from the lobby, this will be set to the id of the user who initiated the kick.
        pub making_change: BigInt,

        /// "ChatMemberStateChange" values.
        pub member_state_change: ChatMemberStateChange,
    }

    #[napi]
    pub struct LobbyChatMsgUpdate {
        pub steam_idlobby: BigInt,
        pub steam_iduser: BigInt,
        pub chat_entry_type: u8,
        pub chat_id: u32,
    }

    #[napi]
    pub struct P2PSessionRequest {
        /// The steam ID of the user requesting a p2p
        /// session
        pub remote: BigInt,
    }

    #[napi]
    pub struct P2PSessionConnectFail {
        pub remote: BigInt,
        pub error: u8,
    }

    #[napi]
    pub struct RelayNetworkStatus {
        pub availability: i32,
        pub is_ping_measurement_in_progress: bool,
        pub network_config: i32,
        pub any_relay: i32,

        pub debugging_message: String,
    }

    #[napi(ts_generic_types = "C extends keyof import('./callbacks').CallbackReturns")]
    pub fn register(
        #[napi(ts_arg_type = "C")] steam_callback: SteamCallback,
        #[napi(ts_arg_type = "(value: import('./callbacks').CallbackReturns[C]) => void")] handler: JsFunction,
    ) -> Handle {
        let handle = match steam_callback {
            SteamCallback::PersonaStateChange => {
                let threadsafe_handler: ThreadsafeFunction<
                    PersonaStateChange,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::PersonaStateChange| {
                    threadsafe_handler.call(
                        PersonaStateChange {
                            steam_id: BigInt::from(value.steam_id.raw()),
                            flags: value.flags.bits(),
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::SteamServersConnected => {
                let threadsafe_handler: ThreadsafeFunction<
                    serde_json::Value,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                register_callback::<steamworks::SteamServersConnected>(threadsafe_handler)
            }
            SteamCallback::SteamServersDisconnected => {
                let threadsafe_handler: ThreadsafeFunction<
                    serde_json::Value,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                register_callback::<steamworks::SteamServersDisconnected>(threadsafe_handler)
            }
            SteamCallback::SteamServerConnectFailure => {
                let threadsafe_handler: ThreadsafeFunction<
                    serde_json::Value,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                register_callback::<steamworks::SteamServerConnectFailure>(threadsafe_handler)
            }
            SteamCallback::LobbyDataUpdate => {
                let threadsafe_handler: ThreadsafeFunction<LobbyDataUpdate, ErrorStrategy::Fatal> =
                    handler
                        .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                        .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::LobbyDataUpdate| {
                    threadsafe_handler.call(
                        LobbyDataUpdate {
                            lobby: BigInt::from(value.lobby.raw()),
                            member: BigInt::from(value.member.raw()),
                            success: value.success,
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::LobbyChatUpdate => {
                let threadsafe_handler: ThreadsafeFunction<LobbyChatUpdate, ErrorStrategy::Fatal> =
                    handler
                        .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                        .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::LobbyChatUpdate| {
                    threadsafe_handler.call(
                        LobbyChatUpdate {
                            lobby: BigInt::from(value.lobby.raw()),
                            user_changed: BigInt::from(value.user_changed.raw()),
                            making_change: BigInt::from(value.making_change.raw()),
                            member_state_change: match value.member_state_change {
                                steamworks::ChatMemberStateChange::Entered => {
                                    ChatMemberStateChange::Entered
                                }
                                steamworks::ChatMemberStateChange::Left => {
                                    ChatMemberStateChange::Left
                                }
                                steamworks::ChatMemberStateChange::Disconnected => {
                                    ChatMemberStateChange::Disconnected
                                }
                                steamworks::ChatMemberStateChange::Kicked => {
                                    ChatMemberStateChange::Kicked
                                }
                                steamworks::ChatMemberStateChange::Banned => {
                                    ChatMemberStateChange::Banned
                                }
                            },
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::LobbyChatMessage => {
                let threadsafe_handler: ThreadsafeFunction<
                    LobbyChatMsgUpdate,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::LobbyChatMsgUpdate| {
                    threadsafe_handler.call(
                        LobbyChatMsgUpdate {
                            steam_idlobby: BigInt::from(value.steam_idlobby),
                            steam_iduser: BigInt::from(value.steam_iduser),
                            chat_entry_type: value.chat_entry_type,
                            chat_id: value.chat_id,
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::P2PSessionRequest => {
                let threadsafe_handler: ThreadsafeFunction<
                    P2PSessionRequest,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::P2PSessionRequest| {
                    threadsafe_handler.call(
                        P2PSessionRequest {
                            remote: BigInt::from(value.remote.raw()),
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::P2PSessionConnectFail => {
                let threadsafe_handler: ThreadsafeFunction<
                    P2PSessionConnectFail,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                let client = crate::client::get_client();
                client.register_callback(move |value: steamworks::P2PSessionConnectFail| {
                    threadsafe_handler.call(
                        P2PSessionConnectFail {
                            remote: BigInt::from(value.remote.raw()),
                            error: value.error,
                        },
                        ThreadsafeFunctionCallMode::Blocking,
                    );
                })
            }
            SteamCallback::RelayNetworkStatusCallback => {
                let client = crate::client::get_client();

                let threadsafe_handler: ThreadsafeFunction<
                    RelayNetworkStatus,
                    ErrorStrategy::Fatal,
                > = handler
                    .create_threadsafe_function(0, |ctx| Ok(vec![ctx.value]))
                    .unwrap();

                let toi32 = |arg: steamworks::networking_types::NetworkingAvailabilityResult| {
                    arg.ok()
                        .map_or_else(|| arg.err().unwrap() as i32, |o| o as i32)
                };

                client.networking_utils().relay_network_status_callback(
                    move |f: steamworks::networking_utils::RelayNetworkStatus| {
                        threadsafe_handler.call(
                            RelayNetworkStatus {
                                availability: toi32(f.availability()),
                                is_ping_measurement_in_progress: f
                                    .is_ping_measurement_in_progress(),
                                network_config: toi32(f.network_config()),
                                any_relay: toi32(f.any_relay()),
                                debugging_message: f.debugging_message().to_string(),
                            },
                            ThreadsafeFunctionCallMode::Blocking,
                        );
                    },
                )
            }
        };

        Handle {
            handle: Some(handle),
        }
    }

    fn register_callback<C>(
        threadsafe_handler: ThreadsafeFunction<serde_json::Value, ErrorStrategy::Fatal>,
    ) -> steamworks::CallbackHandle
    where
        C: steamworks::Callback + serde::Serialize,
    {
        let client = crate::client::get_client();
        client.register_callback(move |value: C| {
            let value = serde_json::to_value(&value).unwrap();
            threadsafe_handler.call(value, ThreadsafeFunctionCallMode::Blocking);
        })
    }
}
