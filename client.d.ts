export function init(appId: number): void
export function restartAppIfNecessary(appId: number): boolean
export function runCallbacks(): void
export interface PlayerSteamId {
  steamId64: bigint
  steamId32: string
  accountId: number
}
export namespace achievement {
  export function activate(achievement: string): boolean
  export function isActivated(achievement: string): boolean
}
export namespace apps {
  export function isSubscribedApp(appId: number): boolean
  export function isAppInstalled(appId: number): boolean
  export function isDlcInstalled(appId: number): boolean
  export function isSubscribedFromFreeWeekend(): boolean
  export function isVacBanned(): boolean
  export function isCybercafe(): boolean
  export function isLowViolence(): boolean
  export function isSubscribed(): boolean
  export function appBuildId(): number
  export function appInstallDir(appId: number): string
  export function appOwner(): PlayerSteamId
  export function availableGameLanguages(): Array<string>
  export function currentGameLanguage(): string
  export function currentBetaName(): string | null
}
export namespace auth {
  /** @param timeoutSeconds - The number of seconds to wait for the ticket to be validated. Default value is 10 seconds. */
  export function getSessionTicket(timeoutSeconds?: number | undefined | null): Promise<Ticket>
  export class Ticket {
    cancel(): void
    getBytes(): Uint8Array
  }
}
export namespace callback {
  export const enum SteamCallback {
    PersonaStateChange = 0,
    SteamServersConnected = 1,
    SteamServersDisconnected = 2,
    SteamServerConnectFailure = 3,
    LobbyDataUpdate = 4,
    LobbyChatUpdate = 5,
    P2PSessionRequest = 6,
    P2PSessionConnectFail = 7
  }
  export function register<C extends keyof import('./callbacks').CallbackReturns>(steamCallback: C, handler: (value: import('./callbacks').CallbackReturns[C]) => void): Handle
  export class Handle {
    disconnect(): void
  }
}
export namespace cloud {
  export function isEnabledForAccount(): boolean
  export function isEnabledForApp(): boolean
  export function readFile(name: string): string
  export function writeFile(name: string, content: string): boolean
  export function deleteFile(name: string): boolean
  export function fileExists(name: string): boolean
}
export namespace input {
  export interface AnalogActionVector {
    x: number
    y: number
  }
  export function init(): void
  export function getControllers(): Array<Controller>
  export function getActionSet(actionSetName: string): bigint
  export function getDigitalAction(actionName: string): bigint
  export function getAnalogAction(actionName: string): bigint
  export function shutdown(): void
  export class Controller {
    activateActionSet(actionSetHandle: bigint): void
    isDigitalActionPressed(actionHandle: bigint): boolean
    getAnalogActionVector(actionHandle: bigint): AnalogActionVector
  }
}
export namespace localplayer {
  export function getSteamId(): PlayerSteamId
  export function getName(): string
  export function getLevel(): number
  /** @returns the 2 digit ISO 3166-1-alpha-2 format country code which client is running in, e.g. "US" or "UK". */
  export function getIpCountry(): string
  export function setRichPresence(key: string, value?: string | undefined | null): void
  export function getPersonState(steamId64: bigint): number
  export function getPersonAvatar(steamId64: bigint, size: number): Array<number> | null
}
export namespace matchmaking {
  export const enum LobbyType {
    Private = 0,
    FriendsOnly = 1,
    Public = 2,
    Invisible = 3
  }
  export const enum LobbyComparison {
    EqualOrLessThan = -2,
    LessThan = -1,
    Equal = 0,
    GreaterThan = 1,
    EqualOrGreaterThan = 2,
    NotEqual = 3
  }
  export const enum LobbyDistanceFilter {
    Close = 0,
    Default = 1,
    Far = 2,
    Worldwide = 3
  }
  export const enum EFriendFlags {
    KEfriendFlagNone = 0,
    KEfriendFlagBlocked = 1,
    KEfriendFlagFriendshipRequested = 2,
    KEfriendFlagImmediate = 4,
    KEfriendFlagClanMember = 8,
    KEfriendFlagOnGameServer = 16,
    KEfriendFlagRequestingFriendship = 128,
    KEfriendFlagRequestingInfo = 256,
    KEfriendFlagIgnored = 512,
    KEfriendFlagIgnoredFriend = 1024,
    KEfriendFlagChatMember = 4096,
    KEfriendFlagAll = 65535
  }
  export function createLobby(lobbyType: LobbyType, maxMembers: number): Promise<Lobby>
  export function joinJobby(lobbyId: bigint): Promise<Lobby>
  export function setFindLobbiesStringFilter(key: string, value: string, comp: LobbyComparison): void
  export function setFindLobbiesNumFilter(key: string, value: number, comp: LobbyComparison): void
  export function setFindLobbiesLobbyDistanceFilter(comp: LobbyDistanceFilter): void
  export function requestLobbyData(lobbyId: bigint): boolean
  export function getLobbyData(lobbyId: bigint, key: string): string | null
  export function getLobbyMemberData(lobbyId: bigint, userId: bigint, key: string): string | null
  export function getOwner(lobbyId: bigint): bigint
  export function getLobbies(): Promise<Array<Lobby>>
  export function setLobbyMemberData(lobbyId: bigint, key: string, value: string): void
  export function setLobbyData(lobbyId: bigint, key: string, value: string): boolean
  export function leave(lobbyId: bigint): void
  export function sendLobbyChatMsg(lobbyId: bigint, body: string, cap: number): boolean
  export function hasFriend(steamIdfriend: bigint, iFriendFlags: EFriendFlags): boolean
  export function getMemberCount(lobbyId: bigint): bigint
  export function getMembers(lobbyId: bigint): Array<PlayerSteamId>
  /** Get an object containing all the lobby data */
  export function getFullData(lobbyId: bigint): Record<string, string>
  export class Lobby {
    id: bigint
    join(): Promise<Lobby>
    leave(): void
    openInviteDialog(): void
    getMemberCount(): bigint
    getMemberLimit(): bigint | null
    getMembers(): Array<PlayerSteamId>
    getOwner(): PlayerSteamId
    setJoinable(joinable: boolean): boolean
    getData(key: string): string | null
    setData(key: string, value: string): boolean
    setMemberData(key: string, value: string): void
    deleteData(key: string): boolean
    sendLobbyChatMsg(body: string, cap: number): boolean
    /** Get an object containing all the lobby data */
    getFullData(): Record<string, string>
    /** Merge current lobby data with provided data in a single batch */
    mergeFullData(data: Record<string, string>): boolean
  }
}
export namespace networking {
  export interface P2PPacket {
    data: Buffer
    size: number
    steamId: PlayerSteamId
  }
  /** The method used to send a packet */
  export const enum SendType {
    /**
     * Send the packet directly over udp.
     *
     * Can't be larger than 1200 bytes
     */
    Unreliable = 0,
    /**
     * Like `Unreliable` but doesn't buffer packets
     * sent before the connection has started.
     */
    UnreliableNoDelay = 1,
    /**
     * Reliable packet sending.
     *
     * Can't be larger than 1 megabyte.
     */
    Reliable = 2,
    /**
     * Like `Reliable` but applies the nagle
     * algorithm to packets being sent
     */
    ReliableWithBuffering = 3
  }
  export function sendP2PPacket(steamId64: bigint, sendType: SendType, data: Buffer): boolean
  export function isP2PPacketAvailable(): number
  export function readP2PPacket(size: number): P2PPacket
  export function acceptP2PSession(steamId64: bigint): void
}
export namespace stats {
  export function getInt(name: string): number | null
  export function setInt(name: string, value: number): boolean
  export function store(): boolean
  export function resetAll(achievementsToo: boolean): boolean
}
export namespace workshop {
  export interface UgcResult {
    itemId: bigint
    needsToAcceptAgreement: boolean
  }
  export interface UgcUpdate {
    title?: string
    description?: string
    changeNote?: string
    previewPath?: string
    contentPath?: string
    tags?: Array<string>
  }
  export interface InstallInfo {
    folder: string
    sizeOnDisk: bigint
    timestamp: number
  }
  export interface DownloadInfo {
    current: bigint
    total: bigint
  }
  export function createItem(): Promise<UgcResult>
  export function updateItem(itemId: bigint, updateDetails: UgcUpdate): Promise<UgcResult>
  /**
   * Subscribe to a workshop item. It will be downloaded and installed as soon as possible.
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#SubscribeItem}
   */
  export function subscribe(itemId: bigint): Promise<void>
  /**
   * Unsubscribe from a workshop item. This will result in the item being removed after the game quits.
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#UnsubscribeItem}
   */
  export function unsubscribe(itemId: bigint): Promise<void>
  /**
   * Gets the current state of a workshop item on this client. States can be combined.
   *
   * @returns a number with the current item state, e.g. 9
   * 9 = 1 (The current user is subscribed to this item) + 8 (The item needs an update)
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#GetItemState}
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#EItemState}
   */
  export function state(itemId: bigint): number
  /**
   * Gets info about currently installed content on the disc for workshop item.
   *
   * @returns an object with the the properties {folder, size_on_disk, timestamp}
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#GetItemInstallInfo}
   */
  export function installInfo(itemId: bigint): InstallInfo | null
  /**
   * Get info about a pending download of a workshop item.
   *
   * @returns an object with the properties {current, total}
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#GetItemDownloadInfo}
   */
  export function downloadInfo(itemId: bigint): DownloadInfo | null
  /**
   * Download or update a workshop item.
   *
   * @param highPriority - If high priority is true, start the download in high priority mode, pausing any existing in-progress Steam downloads and immediately begin downloading this workshop item.
   * @returns true or false
   *
   * {@link https://partner.steamgames.com/doc/api/ISteamUGC#DownloadItem}
   */
  export function download(itemId: bigint, highPriority: boolean): boolean
}
