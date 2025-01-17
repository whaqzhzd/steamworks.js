use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;

#[napi]
pub mod matchmaking {
    use crate::api::localplayer::PlayerSteamId;
    use napi::bindgen_prelude::FromNapiValue;
    use napi::bindgen_prelude::{BigInt, Error, ToNapiValue};
    use std::collections::HashMap;
    use steamworks::LobbyId;
    use tokio::sync::oneshot;

    #[napi]
    pub enum LobbyType {
        Private,
        FriendsOnly,
        Public,
        Invisible,
    }

    #[napi]
    pub struct Lobby {
        pub id: BigInt,
        lobby_id: LobbyId,
    }

    #[napi]
    pub struct ChatMessage {
        pub chat: String,
        pub user: BigInt,
    }

    #[napi]
    pub enum LobbyComparison {
        EqualOrLessThan = -2,
        LessThan = -1,
        Equal = 0,
        GreaterThan = 1,
        EqualOrGreaterThan = 2,
        NotEqual = 3,
    }

    #[napi]
    pub enum LobbyDistanceFilter {
        Close,
        Default,
        Far,
        Worldwide,
    }

    #[napi]
    pub enum EFriendFlags {
        KEfriendFlagNone = 0x00,
        KEfriendFlagBlocked = 0x01,
        KEfriendFlagFriendshipRequested = 0x02,
        KEfriendFlagImmediate = 0x04, // "regular" friend
        KEfriendFlagClanMember = 0x08,
        KEfriendFlagOnGameServer = 0x10,
        // k_EFriendFlagHasPlayedWith	= 0x20,	// not currently used
        // k_EFriendFlagFriendOfFriend	= 0x40, // not currently used
        KEfriendFlagRequestingFriendship = 0x80,
        KEfriendFlagRequestingInfo = 0x100,
        KEfriendFlagIgnored = 0x200,
        KEfriendFlagIgnoredFriend = 0x400,
        // k_EFriendFlagSuggested		= 0x800,	// not used
        KEfriendFlagChatMember = 0x1000,
        KEfriendFlagAll = 0xFFFF,
    }

    #[napi]
    impl Lobby {
        #[napi]
        pub async fn join(&self) -> Result<Lobby, Error> {
            match join_jobby(self.id.clone()).await {
                Ok(lobby) => Ok(lobby),
                Err(e) => Err(e),
            }
        }

        #[napi]
        pub fn leave(&self) {
            let client = crate::client::get_client();
            client.matchmaking().leave_lobby(self.lobby_id);
        }

        #[napi]
        pub fn open_invite_dialog(&self) {
            let client = crate::client::get_client();
            client.friends().activate_invite_dialog(self.lobby_id);
        }

        #[napi]
        pub fn get_member_count(&self) -> usize {
            let client = crate::client::get_client();
            client.matchmaking().lobby_member_count(self.lobby_id)
        }

        #[napi]
        pub fn get_member_limit(&self) -> Option<usize> {
            let client = crate::client::get_client();
            client.matchmaking().lobby_member_limit(self.lobby_id)
        }

        #[napi]
        pub fn get_members(&self) -> Vec<PlayerSteamId> {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .lobby_members(self.lobby_id)
                .into_iter()
                .map(|member| PlayerSteamId::from_steamid(member))
                .collect()
        }

        #[napi]
        pub fn get_owner(&self) -> PlayerSteamId {
            let client = crate::client::get_client();
            PlayerSteamId::from_steamid(client.matchmaking().lobby_owner(self.lobby_id))
        }

        #[napi]
        pub fn set_joinable(&self, joinable: bool) -> bool {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .set_lobby_joinable(self.lobby_id, joinable)
        }

        #[napi]
        pub fn get_data(&self, key: String) -> Option<String> {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .lobby_data(self.lobby_id, &key)
                .map(|s| s.to_string())
        }

        #[napi]
        pub fn set_data(&self, key: String, value: String) -> bool {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .set_lobby_data(self.lobby_id, &key, &value)
        }

        #[napi]
        pub fn set_member_data(&self, key: String, value: String) {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .set_lobby_member_data(self.lobby_id, &key, &value)
        }

        #[napi]
        pub fn delete_data(&self, key: String) -> bool {
            let client = crate::client::get_client();
            client.matchmaking().delete_lobby_data(self.lobby_id, &key)
        }

        #[napi]
        pub fn send_lobby_chat_msg(&self, body: String, cap: i32) -> bool {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .send_lobby_chat_msg(self.lobby_id, body.as_str(), cap)
        }

        /// Get an object containing all the lobby data
        #[napi]
        pub fn get_full_data(&self) -> HashMap<String, String> {
            let client = crate::client::get_client();

            let mut data = HashMap::new();

            let count = client.matchmaking().lobby_data_count(self.lobby_id);
            for i in 0..count {
                let maybe_lobby_data = client.matchmaking().lobby_data_by_index(self.lobby_id, i);

                if let Some((key, value)) = maybe_lobby_data {
                    data.insert(key, value);
                }
            }

            return data;
        }

        /// Merge current lobby data with provided data in a single batch
        #[napi]
        pub fn merge_full_data(&self, data: HashMap<String, String>) -> bool {
            let client = crate::client::get_client();

            for (key, value) in data {
                client
                    .matchmaking()
                    .set_lobby_data(self.lobby_id, &key, &value);
            }

            return true;
        }
    }

    #[napi]
    pub async fn create_lobby(lobby_type: LobbyType, max_members: u32) -> Result<Lobby, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().create_lobby(
            match lobby_type {
                LobbyType::Private => steamworks::LobbyType::Private,
                LobbyType::FriendsOnly => steamworks::LobbyType::FriendsOnly,
                LobbyType::Public => steamworks::LobbyType::Public,
                LobbyType::Invisible => steamworks::LobbyType::Invisible,
            },
            max_members,
            |result| {
                tx.send(result).unwrap();
            },
        );

        let result = rx.await.unwrap();
        match result {
            Ok(lobby_id) => {
                #[cfg(feature = "dev")]
                dbg!(lobby_id);

                Ok(Lobby {
                    id: BigInt::from(lobby_id.raw()),
                    lobby_id,
                })
            }
            Err(e) => Err(Error::from_reason(e.to_string())),
        }
    }

    #[napi]
    pub async fn join_jobby(lobby_id: BigInt) -> Result<Lobby, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().join_lobby(
            steamworks::LobbyId::from_raw(lobby_id.get_u64().1),
            |result| {
                tx.send(result).unwrap();
            },
        );

        let result = rx.await.unwrap();
        match result {
            Ok(lobby_id) => Ok(Lobby {
                id: BigInt::from(lobby_id.raw()),
                lobby_id,
            }),
            Err(_) => Err(Error::from_reason("Failed to join lobby".to_string())),
        }
    }

    #[napi]
    pub fn set_find_lobbies_string_filter(key: String, value: String, comp: LobbyComparison) {
        let client = crate::client::get_client();
        client.matchmaking().add_lobby_string_filter(
            key,
            value,
            match comp {
                LobbyComparison::EqualOrLessThan => steamworks::LobbyComparison::EqualOrLessThan,
                LobbyComparison::LessThan => steamworks::LobbyComparison::LessThan,
                LobbyComparison::Equal => steamworks::LobbyComparison::Equal,
                LobbyComparison::GreaterThan => steamworks::LobbyComparison::GreaterThan,
                LobbyComparison::EqualOrGreaterThan => {
                    steamworks::LobbyComparison::EqualOrGreaterThan
                }
                LobbyComparison::NotEqual => steamworks::LobbyComparison::NotEqual,
            },
        );
    }

    #[napi]
    pub fn set_find_lobbies_num_filter(key: String, value: i32, comp: LobbyComparison) {
        let client = crate::client::get_client();
        client.matchmaking().add_lobby_num_filter(
            key,
            value,
            match comp {
                LobbyComparison::EqualOrLessThan => steamworks::LobbyComparison::EqualOrLessThan,
                LobbyComparison::LessThan => steamworks::LobbyComparison::LessThan,
                LobbyComparison::Equal => steamworks::LobbyComparison::Equal,
                LobbyComparison::GreaterThan => steamworks::LobbyComparison::GreaterThan,
                LobbyComparison::EqualOrGreaterThan => {
                    steamworks::LobbyComparison::EqualOrGreaterThan
                }
                LobbyComparison::NotEqual => steamworks::LobbyComparison::NotEqual,
            },
        );
    }

    #[napi]
    pub fn set_find_lobbies_lobby_distance_filter(comp: LobbyDistanceFilter) {
        let client = crate::client::get_client();
        client.matchmaking().add_lobby_distance_filter(match comp {
            LobbyDistanceFilter::Close => steamworks::LobbyDistanceFilter::Close,
            LobbyDistanceFilter::Default => steamworks::LobbyDistanceFilter::Default,
            LobbyDistanceFilter::Far => steamworks::LobbyDistanceFilter::Far,
            LobbyDistanceFilter::Worldwide => steamworks::LobbyDistanceFilter::Worldwide,
        });
    }

    #[napi]
    pub fn request_lobby_data(lobby_id: BigInt) -> bool {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .request_lobby_data(LobbyId::from_raw(lobby_id.get_u64().1))
    }

    #[napi]
    pub fn get_lobby_data(lobby_id: BigInt, key: String) -> Option<String> {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_data(LobbyId::from_raw(lobby_id.get_u64().1), key.as_str())
            .map(|s| s.to_string())
    }

    #[napi]
    pub fn get_lobby_member_data(lobby_id: BigInt, user_id: BigInt, key: String) -> Option<String> {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_member_data(
                LobbyId::from_raw(lobby_id.get_u64().1),
                LobbyId::from_raw(user_id.get_u64().1),
                key.as_str(),
            )
            .map(|s| s.to_string())
    }

    #[napi]
    pub fn get_owner(lobby_id: BigInt) -> BigInt {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_owner(LobbyId::from_raw(lobby_id.get_u64().1))
            .raw()
            .into()
    }

    #[napi]
    pub async fn get_lobbies() -> Result<Vec<Lobby>, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().request_lobby_list(|lobbies| {
            tx.send(lobbies).unwrap();
        });

        let lobbies = rx.await.unwrap();

        match lobbies {
            Ok(lobbies) => Ok(lobbies
                .iter()
                .map(|lobby_id| Lobby {
                    id: BigInt::from(lobby_id.raw()),
                    lobby_id: *lobby_id,
                })
                .collect()),
            Err(e) => Err(Error::from_reason(e.to_string())),
        }
    }

    #[napi]
    pub fn set_lobby_member_data(lobby_id: BigInt, key: String, value: String) {
        let client = crate::client::get_client();
        client.matchmaking().set_lobby_member_data(
            LobbyId::from_raw(lobby_id.get_u64().1),
            key.as_str(),
            value.as_str(),
        )
    }

    #[napi]
    pub fn set_lobby_data(lobby_id: BigInt, key: String, value: String) -> bool {
        let client = crate::client::get_client();
        client.matchmaking().set_lobby_data(
            LobbyId::from_raw(lobby_id.get_u64().1),
            key.as_str(),
            value.as_str(),
        )
    }

    #[napi]
    pub fn leave(lobby_id: BigInt) {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .leave_lobby(LobbyId::from_raw(lobby_id.get_u64().1));
    }

    #[napi]
    pub fn send_lobby_chat_msg(lobby_id: BigInt, body: String, cap: i32) -> bool {
        let client = crate::client::get_client();
        client.matchmaking().send_lobby_chat_msg(
            LobbyId::from_raw(lobby_id.get_u64().1),
            body.as_str(),
            cap,
        )
    }

    #[napi]
    pub fn has_friend(steam_idfriend: BigInt, i_friend_flags: EFriendFlags) -> bool {
        let client = crate::client::get_client();
        client.friends().has_friends(
            steamworks::SteamId::from_raw(steam_idfriend.get_u64().1),
            match i_friend_flags {
                EFriendFlags::KEfriendFlagNone => steamworks::EFriendFlags::KEfriendFlagNone,
                EFriendFlags::KEfriendFlagBlocked => steamworks::EFriendFlags::KEfriendFlagBlocked,
                EFriendFlags::KEfriendFlagFriendshipRequested => {
                    steamworks::EFriendFlags::KEfriendFlagFriendshipRequested
                }
                EFriendFlags::KEfriendFlagImmediate => {
                    steamworks::EFriendFlags::KEfriendFlagImmediate
                }
                EFriendFlags::KEfriendFlagClanMember => {
                    steamworks::EFriendFlags::KEfriendFlagClanMember
                }
                EFriendFlags::KEfriendFlagOnGameServer => {
                    steamworks::EFriendFlags::KEfriendFlagOnGameServer
                }
                EFriendFlags::KEfriendFlagRequestingFriendship => {
                    steamworks::EFriendFlags::KEfriendFlagRequestingFriendship
                }
                EFriendFlags::KEfriendFlagRequestingInfo => {
                    steamworks::EFriendFlags::KEfriendFlagRequestingInfo
                }
                EFriendFlags::KEfriendFlagIgnored => steamworks::EFriendFlags::KEfriendFlagIgnored,
                EFriendFlags::KEfriendFlagIgnoredFriend => {
                    steamworks::EFriendFlags::KEfriendFlagIgnoredFriend
                }
                EFriendFlags::KEfriendFlagChatMember => {
                    steamworks::EFriendFlags::KEfriendFlagChatMember
                }
                EFriendFlags::KEfriendFlagAll => steamworks::EFriendFlags::KEfriendFlagAll,
            },
        )
    }

    #[napi]
    pub fn get_member_count(lobby_id: BigInt) -> usize {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_member_count(LobbyId::from_raw(lobby_id.get_u64().1))
    }

    #[napi]
    pub fn get_members(lobby_id: BigInt) -> Vec<PlayerSteamId> {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_members(LobbyId::from_raw(lobby_id.get_u64().1))
            .into_iter()
            .map(|member| PlayerSteamId::from_steamid(member))
            .collect()
    }

    /// Get an object containing all the lobby data
    #[napi]
    pub fn get_full_data(lobby_id: BigInt) -> HashMap<String, String> {
        let client = crate::client::get_client();
        let lobby = LobbyId::from_raw(lobby_id.get_u64().1);

        let mut data = HashMap::new();

        let count = client.matchmaking().lobby_data_count(lobby);
        for i in 0..count {
            let maybe_lobby_data = client.matchmaking().lobby_data_by_index(lobby, i);

            if let Some((key, value)) = maybe_lobby_data {
                data.insert(key, value);
            }
        }

        return data;
    }

    /// Get Chat Message
    #[napi]
    pub fn get_chat_message(steam_idlobby: BigInt, chat_id: i32) -> ChatMessage {
        let client = crate::client::get_client();

        let msg = client
            .matchmaking()
            .get_chat_message(steam_idlobby.get_u64().1, chat_id);
        ChatMessage {
            chat: msg.0,
            user: BigInt::from(msg.1.raw()),
        }
    }
}
