use super::*;
#[cfg(test)]
use serial_test::serial;

/// Access to the steam user interface
pub struct User<Manager> {
    pub(crate) user: *mut sys::ISteamUser,
    pub(crate) _inner: Arc<Inner<Manager>>,
}

impl<Manager> User<Manager> {
    /// Returns the steam id of the current user
    pub fn steam_id(&self) -> SteamId {
        unsafe { SteamId(sys::SteamAPI_ISteamUser_GetSteamID(self.user)) }
    }

    /// Returns the level of the current user
    pub fn level(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUser_GetPlayerSteamLevel(self.user) as u32 }
    }

    /// Retrieve an authentication session ticket that can be sent
    /// to an entity that wishes to verify you.
    ///
    /// This ticket should not be reused.
    ///
    /// When creating ticket for use by the web API you should wait
    /// for the `AuthSessionTicketResponse` event before trying to
    /// use the ticket.
    ///
    /// When the multiplayer session terminates you must call
    /// `cancel_authentication_ticket`
    pub fn authentication_session_ticket(&self) -> (AuthTicket, Vec<u8>) {
        unsafe {
            let mut ticket = vec![0; 1024];
            let mut ticket_len = 0;
            let auth_ticket = sys::SteamAPI_ISteamUser_GetAuthSessionTicket(
                self.user,
                ticket.as_mut_ptr() as *mut _,
                1024,
                &mut ticket_len,
            );
            ticket.truncate(ticket_len as usize);
            (AuthTicket(auth_ticket), ticket)
        }
    }

    /// Cancels an authentication session ticket received from
    /// `authentication_session_ticket`.
    ///
    /// This should be called when you are no longer playing with
    /// the specified entity.
    pub fn cancel_authentication_ticket(&self, ticket: AuthTicket) {
        unsafe {
            sys::SteamAPI_ISteamUser_CancelAuthTicket(self.user, ticket.0);
        }
    }

    /// Authenticate the ticket from the steam ID to make sure it is
    /// valid and not reused.
    ///
    /// A `ValidateAuthTicketResponse` callback will be fired if
    /// the entity goes offline or cancels the ticket.
    ///
    /// When the multiplayer session terminates you must call
    /// `end_authentication_session`
    pub fn begin_authentication_session(
        &self,
        user: SteamId,
        ticket: &[u8],
    ) -> Result<(), AuthSessionError> {
        unsafe {
            let res = sys::SteamAPI_ISteamUser_BeginAuthSession(
                self.user,
                ticket.as_ptr() as *const _,
                ticket.len() as _,
                user.0,
            );
            Err(match res {
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultOK => return Ok(()),
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultInvalidTicket => {
                    AuthSessionError::InvalidTicket
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultDuplicateRequest => {
                    AuthSessionError::DuplicateRequest
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultInvalidVersion => {
                    AuthSessionError::InvalidVersion
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultGameMismatch => {
                    AuthSessionError::GameMismatch
                }
                sys::EBeginAuthSessionResult::k_EBeginAuthSessionResultExpiredTicket => {
                    AuthSessionError::ExpiredTicket
                }
                _ => unreachable!(),
            })
        }
    }

    /// Ends an authentication session that was started with
    /// `begin_authentication_session`.
    ///
    /// This should be called when you are no longer playing with
    /// the specified entity.
    pub fn end_authentication_session(&self, user: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamUser_EndAuthSession(self.user, user.0);
        }
    }

    pub fn set_duration_control_online_state(&self, state: DurationControlOnlineState) -> bool {
        unsafe {
            sys::SteamAPI_ISteamUser_BSetDurationControlOnlineState(
                self.user,
                match state {
                    DurationControlOnlineState::DurationControlOnlineStateInvalid => {
                        sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Invalid
                    }
                    DurationControlOnlineState::DurationControlOnlineStateOffline => {
                        sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Offline
                    }
                    DurationControlOnlineState::DurationControlOnlineStateOnline => {
                        sys::EDurationControlOnlineState::k_EDurationControlOnlineState_Online
                    }
                    DurationControlOnlineState::DurationControlOnlineStateOnlineHighPri => {
                        sys::EDurationControlOnlineState::k_EDurationControlOnlineState_OnlineHighPri
                    }
                },
            )
        }
    }
}

#[derive(Debug)]
pub enum DurationControlOnlineState {
    DurationControlOnlineStateInvalid = 0,       // nil value
    DurationControlOnlineStateOffline = 1, // currently in offline play - single-player, offline co-op, etc.
    DurationControlOnlineStateOnline = 2,  // currently in online play
    DurationControlOnlineStateOnlineHighPri = 3, // currently in online play and requests not to be interrupted
}

/// Errors from `begin_authentication_session`
#[derive(Debug, Error)]
pub enum AuthSessionError {
    /// The ticket is invalid
    #[error("invalid ticket")]
    InvalidTicket,
    /// A ticket has already been submitted for this steam ID
    #[error("duplicate ticket request")]
    DuplicateRequest,
    /// The ticket is from an incompatible interface version
    #[error("incompatible interface version")]
    InvalidVersion,
    /// The ticket is not for this game
    #[error("incorrect game for ticket")]
    GameMismatch,
    /// The ticket has expired
    #[error("ticket has expired")]
    ExpiredTicket,
}

#[test]
#[serial]
fn test() {
    let (client, single) = Client::init().unwrap();
    let user = client.user();

    let _cb = client
        .register_callback(|v: AuthSessionTicketResponse| println!("Got response: {:?}", v.result));
    let _cb = client.register_callback(|v: ValidateAuthTicketResponse| println!("{:?}", v));

    let id = user.steam_id();
    let (auth, ticket) = user.authentication_session_ticket();

    println!("{:?}", user.begin_authentication_session(id, &ticket));

    for _ in 0..20 {
        single.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    println!("END");

    user.cancel_authentication_ticket(auth);

    for _ in 0..20 {
        single.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(50));
    }

    user.end_authentication_session(id);
}

/// A handle for an authentication ticket that can be used to cancel
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthTicket(pub(crate) sys::HAuthTicket);

/// Called when generating a authentication session ticket.
///
/// This can be used to verify the ticket was created successfully.
pub struct AuthSessionTicketResponse {
    /// The ticket in question
    pub ticket: AuthTicket,
    /// The result of generating the ticket
    pub result: SResult<()>,
}

unsafe impl Callback for AuthSessionTicketResponse {
    const ID: i32 = 163;
    const SIZE: i32 = ::std::mem::size_of::<sys::GetAuthSessionTicketResponse_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::GetAuthSessionTicketResponse_t);
        AuthSessionTicketResponse {
            ticket: AuthTicket(val.m_hAuthTicket),
            result: if val.m_eResult == sys::EResult::k_EResultOK {
                Ok(())
            } else {
                Err(val.m_eResult.into())
            },
        }
    }
}

/// Called when an authentication ticket has been
/// validated.
#[derive(Debug)]
pub struct ValidateAuthTicketResponse {
    /// The steam id of the entity that provided the ticket
    pub steam_id: SteamId,
    /// The result of the validation
    pub response: Result<(), AuthSessionValidateError>,
    /// The steam id of the owner of the game. Differs from
    /// `steam_id` if the game is borrowed.
    pub owner_steam_id: SteamId,
}

unsafe impl Callback for ValidateAuthTicketResponse {
    const ID: i32 = 143;
    const SIZE: i32 = ::std::mem::size_of::<sys::ValidateAuthTicketResponse_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::ValidateAuthTicketResponse_t);
        ValidateAuthTicketResponse {
            steam_id: SteamId(val.m_SteamID.m_steamid.m_unAll64Bits),
            owner_steam_id: SteamId(val.m_OwnerSteamID.m_steamid.m_unAll64Bits),
            response: match val.m_eAuthSessionResponse {
                sys::EAuthSessionResponse::k_EAuthSessionResponseOK => Ok(()),
                sys::EAuthSessionResponse::k_EAuthSessionResponseUserNotConnectedToSteam => {
                    Err(AuthSessionValidateError::UserNotConnectedToSteam)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseNoLicenseOrExpired => {
                    Err(AuthSessionValidateError::NoLicenseOrExpired)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseVACBanned => {
                    Err(AuthSessionValidateError::VACBanned)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseLoggedInElseWhere => {
                    Err(AuthSessionValidateError::LoggedInElseWhere)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseVACCheckTimedOut => {
                    Err(AuthSessionValidateError::VACCheckTimedOut)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketCanceled => {
                    Err(AuthSessionValidateError::AuthTicketCancelled)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketInvalidAlreadyUsed => {
                    Err(AuthSessionValidateError::AuthTicketInvalidAlreadyUsed)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponseAuthTicketInvalid => {
                    Err(AuthSessionValidateError::AuthTicketInvalid)
                }
                sys::EAuthSessionResponse::k_EAuthSessionResponsePublisherIssuedBan => {
                    Err(AuthSessionValidateError::PublisherIssuedBan)
                }
                _ => unreachable!(),
            },
        }
    }
}

/// Called when a connection to the Steam servers is made.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersConnected;

unsafe impl Callback for SteamServersConnected {
    const ID: i32 = 101;
    const SIZE: i32 = ::std::mem::size_of::<sys::SteamServersConnected_t>() as i32;

    unsafe fn from_raw(_: *mut c_void) -> Self {
        SteamServersConnected
    }
}

/// Called when the connection to the Steam servers is lost.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersDisconnected {
    /// The reason we were disconnected from the Steam servers
    pub reason: SteamError,
}

unsafe impl Callback for SteamServersDisconnected {
    const ID: i32 = 103;
    const SIZE: i32 = ::std::mem::size_of::<sys::SteamServersDisconnected_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::SteamServersDisconnected_t);
        SteamServersDisconnected {
            reason: val.m_eResult.into(),
        }
    }
}

/// Called when the connection to the Steam servers fails.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServerConnectFailure {
    /// The reason we failed to connect to the Steam servers
    pub reason: SteamError,
    /// Whether we are still retrying the connection.
    pub still_retrying: bool,
}

unsafe impl Callback for SteamServerConnectFailure {
    const ID: i32 = 102;
    const SIZE: i32 = ::std::mem::size_of::<sys::SteamServerConnectFailure_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::SteamServerConnectFailure_t);
        SteamServerConnectFailure {
            reason: val.m_eResult.into(),
            still_retrying: val.m_bStillRetrying,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyGameCreated {
    pub ul_steam_idlobby: u64,       // the lobby we were in
    pub ul_steam_idgame_server: u64, // the new game server that has been created or found for the lobby members
    pub un_ip: u32,                  // IP & Port of the game server (if any)
    pub us_port: u16,
}

unsafe impl Callback for LobbyGameCreated {
    const ID: i32 = 509;
    const SIZE: i32 = ::std::mem::size_of::<sys::LobbyGameCreated_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::LobbyGameCreated_t);
        LobbyGameCreated {
            ul_steam_idlobby: val.m_ulSteamIDLobby,
            ul_steam_idgame_server: val.m_ulSteamIDGameServer,
            un_ip: val.m_unIP,
            us_port: val.m_usPort,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameRichPresenceJoinRequested {
    pub steam_idfriend: SteamId,
    pub rgch_connect: String,
}

unsafe impl Callback for GameRichPresenceJoinRequested {
    const ID: i32 = 337;
    const SIZE: i32 = ::std::mem::size_of::<sys::GameRichPresenceJoinRequested_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let val = &mut *(raw as *mut sys::GameRichPresenceJoinRequested_t);
        let m_rgch_connect = CStr::from_ptr(val.m_rgchConnect.as_ptr()).to_owned();
        GameRichPresenceJoinRequested {
            steam_idfriend: SteamId::from_raw(val.m_steamIDFriend.m_steamid.m_unAll64Bits),
            rgch_connect: m_rgch_connect.into_string().unwrap(),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewUrlLaunchParameters;

unsafe impl Callback for NewUrlLaunchParameters {
    const ID: i32 = 1014;
    const SIZE: i32 = ::std::mem::size_of::<sys::NewUrlLaunchParameters_t>() as i32;

    unsafe fn from_raw(raw: *mut c_void) -> Self {
        let _ = &mut *(raw as *mut sys::NewUrlLaunchParameters_t);
        NewUrlLaunchParameters
    }
}

/// Errors from `ValidateAuthTicketResponse`
#[derive(Debug, Error)]
pub enum AuthSessionValidateError {
    /// The user in question is not connected to steam
    #[error("user not connected to steam")]
    UserNotConnectedToSteam,
    /// The license has expired
    #[error("the license has expired")]
    NoLicenseOrExpired,
    /// The user is VAC banned from the game
    #[error("the user is VAC banned from this game")]
    VACBanned,
    /// The user has logged in elsewhere and the session
    /// has been disconnected
    #[error("the user is logged in elsewhere")]
    LoggedInElseWhere,
    /// VAC has been unable to perform anti-cheat checks on this
    /// user
    #[error("VAC check timed out")]
    VACCheckTimedOut,
    /// The ticket has been cancelled by the issuer
    #[error("the authentication ticket has been cancelled")]
    AuthTicketCancelled,
    /// The ticket has already been used
    #[error("the authentication ticket has already been used")]
    AuthTicketInvalidAlreadyUsed,
    /// The ticket is not from a user instance currently connected
    /// to steam
    #[error("the authentication ticket is invalid")]
    AuthTicketInvalid,
    /// The user is banned from the game (not VAC)
    #[error("the user is banned")]
    PublisherIssuedBan,
}
