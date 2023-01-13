use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use steamworks::Client;
use steamworks::SingleClient;

lazy_static! {
    static ref STEAM_CLIENT: Mutex<Option<Client>> = Mutex::new(None);
    static ref START_TIME: SystemTime = SystemTime::now();
}

static mut STEAM_SINGLE: Option<SingleClient> = None;

pub fn has_client() -> bool {
    STEAM_CLIENT.lock().unwrap().is_some()
}

pub fn get_client() -> Client {
    let option = STEAM_CLIENT.lock().unwrap().to_owned();
    option.unwrap()
}

pub fn set_client(client: Client) {
    let mut client_ref = STEAM_CLIENT.lock().unwrap();
    *client_ref = Some(client);
}

pub fn get_single() -> &'static SingleClient {
    unsafe {
        match &STEAM_SINGLE {
            Some(single) => single,
            None => panic!("Steam single not initialized"),
        }
    }
}

pub fn now() -> i64 {
    let since_the_epoch = START_TIME
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let ms = since_the_epoch.as_secs() as i64 * 1000i64
        + (since_the_epoch.subsec_nanos() as f64 / 1_000_000.0) as i64;
    ms
}

pub fn set_single(single: SingleClient) {
    unsafe {
        STEAM_SINGLE = Some(single);
    }
}
