use napi::bindgen_prelude::Error;
use napi::{CallContext, JsNumber, JsObject};
use napi::{Env, JsString};
use napi_derive::napi;
use napi_derive::{js_function, module_exports};
use steamworks::AppId;
use steamworks::Client;
pub mod client;

extern crate rmp_serde as rmps;
extern crate serde;
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

pub mod encrypt;

#[napi]
pub fn init(app_id: u32) -> Result<(), Error> {
    if client::has_client() {
        let initialized_app_id = client::get_client().utils().app_id().0;
        if initialized_app_id != app_id {
            return Err(Error::from_reason(format!(
                "Client already initialized for app id {}",
                app_id
            )));
        } else {
            return Ok(());
        }
    }

    let result = Client::init_app(app_id);
    match result {
        Ok((steam_client, steam_single)) => {
            steam_client.user_stats().request_current_stats();

            steam_client
                .utils()
                .set_warning_callback(|severity, pch_debug_text| {
                    println!("error: {:?}", pch_debug_text);

                    if severity >= 1 {
                        // place to set a breakpoint for catching API errors
                    }
                });

            client::set_client(steam_client);
            client::set_single(steam_single);
            Ok(())
        }
        Err(e) => Err(Error::from_reason(e.to_string())),
    }
}

#[napi]
pub fn restart_app_if_necessary(app_id: u32) -> bool {
    steamworks::restart_app_if_necessary(AppId(app_id))
}

#[napi]
pub fn run_callbacks() {
    client::get_single().run_callbacks();
}

#[module_exports]
fn mod_init(mut exports: JsObject, env: Env) -> napi::Result<()> {
    let global = env.get_global()?;

    let main_modeul = {
        if global.get_named_property::<JsObject>("module").is_ok() {
            // electron renderer
            global.get_named_property::<JsObject>("module").unwrap()
        } else {
            let process: JsObject = global.get_named_property("process").unwrap();
            let argv: JsObject = process.get_named_property("argv").unwrap();
            let leng = argv.get_array_length().unwrap();

            for x in 0..leng {
                let arg: JsString = argv.get_element::<JsString>(x).unwrap();
                if arg.into_utf8()?.as_str()?.contains("--inspect")
                    || arg
                        .into_utf8()?
                        .as_str()?
                        .contains("--remote-debugging-port")
                {
                    return Ok(());
                }
            }

            process
                .get_named_property::<JsObject>("mainModule")
                .unwrap()
        }
    };

    Ok(())
}

pub mod api;

#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    #[test]
    fn test() {
        assert_eq!(Ipv4Addr::from(0), Ipv4Addr::UNSPECIFIED);
    }
}
