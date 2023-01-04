use napi::bindgen_prelude::BigInt;
use napi_derive::napi;
use steamworks::SteamId;

#[napi(object)]
pub struct PlayerSteamId {
    pub steam_id64: BigInt,
    pub steam_id32: String,
    pub account_id: u32,
}

impl PlayerSteamId {
    pub(crate) fn from_steamid(steam_id: SteamId) -> Self {
        Self {
            steam_id64: steam_id.raw().into(),
            steam_id32: steam_id.steamid32(),
            account_id: steam_id.account_id().raw(),
        }
    }
}

#[napi]
pub mod localplayer {
    use steamworks::SteamId;

    use super::PlayerSteamId;

    #[napi]
    pub fn get_steam_id() -> PlayerSteamId {
        let client = crate::client::get_client();
        let steam_id = client.user().steam_id();
        PlayerSteamId::from_steamid(steam_id)
    }

    #[napi]
    pub fn get_name() -> String {
        let client = crate::client::get_client();
        client.friends().name()
    }

    #[napi]
    pub fn get_level() -> u32 {
        let client = crate::client::get_client();
        client.user().level()
    }

    /// @returns the 2 digit ISO 3166-1-alpha-2 format country code which client is running in, e.g. "US" or "UK".
    #[napi]
    pub fn get_ip_country() -> String {
        let client = crate::client::get_client();
        client.utils().ip_country()
    }

    #[napi]
    pub fn set_rich_presence(key: String, value: Option<String>) {
        let client = crate::client::get_client();
        client.friends().set_rich_presence(&key, value.as_deref());
    }

    #[napi]
    pub fn get_person_state(steam_id64: napi::bindgen_prelude::BigInt) -> u8 {
        let client = crate::client::get_client();
        client
            .friends()
            .get_friend(SteamId::from_raw(steam_id64.get_u64().1))
            .state() as u8
    }

    #[napi]
    pub fn get_person_avatar(
        steam_id64: napi::bindgen_prelude::BigInt, 
        size: u8
    ) -> Option<Vec<u8>> {
        let client = crate::client::get_client();
        let friends = client
            .friends()
            .get_friend(SteamId::from_raw(steam_id64.get_u64().1));

        if size == 0 {
            friends.large_avatar()
        } else if size == 1 {
            friends.medium_avatar()
        } else {
            friends.small_avatar()
        }
    }
}
