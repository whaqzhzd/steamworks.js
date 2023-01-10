use napi_derive::napi;

#[napi]
pub mod steamp2p {
    use napi::bindgen_prelude::ToNapiValue;

    #[napi(js_name = "SteamClient")]
    pub struct JsSteamClient {}

    #[napi]
    impl JsSteamClient {
        #[napi(constructor)]
        pub fn new() -> Self {
            JsSteamClient {}
        }
    }
}
