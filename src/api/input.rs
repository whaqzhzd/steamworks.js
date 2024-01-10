use napi_derive::napi;

#[napi]
pub mod input {
    use napi::bindgen_prelude::BigInt;
    use napi::bindgen_prelude::ToNapiValue;

    #[napi]
    #[derive(PartialEq, Eq)]
    pub enum SteamInputType {
        KEsteamInputTypeUnknown = 0,
        KEsteamInputTypeSteamController = 1,
        KEsteamInputTypeXbox360controller = 2,
        KEsteamInputTypeXboxOneController = 3,
        KEsteamInputTypeGenericGamepad = 4,
        KEsteamInputTypePs4controller = 5,
        KEsteamInputTypeAppleMfiController = 6,
        KEsteamInputTypeAndroidController = 7,
        KEsteamInputTypeSwitchJoyConPair = 8,
        KEsteamInputTypeSwitchJoyConSingle = 9,
        KEsteamInputTypeSwitchProController = 10,
        KEsteamInputTypeMobileTouch = 11,
        KEsteamInputTypePs3controller = 12,
        KEsteamInputTypePs5controller = 13,
        KEsteamInputTypeSteamDeckController = 14,
        KEsteamInputTypeCount = 15,
        KEsteamInputTypeMaximumPossibleValue = 255,
    }

    #[napi]
    pub struct Controller {
        pub handle: BigInt,
    }

    #[napi]
    impl Controller {
        #[napi]
        pub fn activate_action_set(&self, action_set_handle: BigInt) {
            let client = crate::client::get_client();
            client
                .input()
                .activate_action_set_handle(self.handle.get_u64().1, action_set_handle.get_u64().1)
        }

        #[napi]
        pub fn is_digital_action_pressed(&self, action_handle: BigInt) -> bool {
            let client = crate::client::get_client();
            client
                .input()
                .get_digital_action_data(self.handle.get_u64().1, action_handle.get_u64().1)
                .bState
        }

        #[napi]
        pub fn get_analog_action_vector(&self, action_handle: BigInt) -> AnalogActionVector {
            let client = crate::client::get_client();
            let data = client
                .input()
                .get_analog_action_data(self.handle.get_u64().1, action_handle.get_u64().1);
            AnalogActionVector {
                x: data.x as f64,
                y: data.y as f64,
            }
        }

        #[napi]
        pub fn get_controller_type(&self) -> SteamInputType {
            let client = crate::client::get_client();
            match client.input().get_controller_type(self.handle.get_u64().1) {
                steamworks::XSteamInputType::KEsteamInputTypeUnknown => {
                    SteamInputType::KEsteamInputTypeUnknown
                }
                steamworks::XSteamInputType::KEsteamInputTypeSteamController => {
                    SteamInputType::KEsteamInputTypeSteamController
                }
                steamworks::XSteamInputType::KEsteamInputTypeXbox360controller => {
                    SteamInputType::KEsteamInputTypeXbox360controller
                }
                steamworks::XSteamInputType::KEsteamInputTypeXboxOneController => {
                    SteamInputType::KEsteamInputTypeXboxOneController
                }
                steamworks::XSteamInputType::KEsteamInputTypeGenericGamepad => {
                    SteamInputType::KEsteamInputTypeGenericGamepad
                }
                steamworks::XSteamInputType::KEsteamInputTypePs4controller => {
                    SteamInputType::KEsteamInputTypePs4controller
                }
                steamworks::XSteamInputType::KEsteamInputTypeAppleMfiController => {
                    SteamInputType::KEsteamInputTypeAppleMfiController
                }
                steamworks::XSteamInputType::KEsteamInputTypeAndroidController => {
                    SteamInputType::KEsteamInputTypeAndroidController
                }
                steamworks::XSteamInputType::KEsteamInputTypeSwitchJoyConPair => {
                    SteamInputType::KEsteamInputTypeSwitchJoyConPair
                }
                steamworks::XSteamInputType::KEsteamInputTypeSwitchJoyConSingle => {
                    SteamInputType::KEsteamInputTypeSwitchJoyConSingle
                }
                steamworks::XSteamInputType::KEsteamInputTypeSwitchProController => {
                    SteamInputType::KEsteamInputTypeSwitchProController
                }
                steamworks::XSteamInputType::KEsteamInputTypeMobileTouch => {
                    SteamInputType::KEsteamInputTypeMobileTouch
                }
                steamworks::XSteamInputType::KEsteamInputTypePs3controller => {
                    SteamInputType::KEsteamInputTypePs3controller
                }
                steamworks::XSteamInputType::KEsteamInputTypePs5controller => {
                    SteamInputType::KEsteamInputTypePs5controller
                }
                steamworks::XSteamInputType::KEsteamInputTypeSteamDeckController => {
                    SteamInputType::KEsteamInputTypeSteamDeckController
                }
                steamworks::XSteamInputType::KEsteamInputTypeCount => {
                    SteamInputType::KEsteamInputTypeCount
                }
                steamworks::XSteamInputType::KEsteamInputTypeMaximumPossibleValue => {
                    SteamInputType::KEsteamInputTypeMaximumPossibleValue
                }
            }
        }
    }

    #[napi(object)]
    pub struct AnalogActionVector {
        pub x: f64,
        pub y: f64,
    }

    #[napi]
    pub fn init() {
        let client = crate::client::get_client();
        client.input().init(false)
    }

    #[napi]
    pub fn get_controllers() -> Vec<Controller> {
        let client = crate::client::get_client();
        client
            .input()
            .get_connected_controllers()
            .into_iter()
            .map(|identity| Controller {
                handle: BigInt::from(identity),
            })
            .collect()
    }

    #[napi]
    pub fn get_action_set(action_set_name: String) -> BigInt {
        let client = crate::client::get_client();
        BigInt::from(client.input().get_action_set_handle(&action_set_name))
    }

    #[napi]
    pub fn get_digital_action(action_name: String) -> BigInt {
        let client = crate::client::get_client();
        BigInt::from(client.input().get_digital_action_handle(&action_name))
    }

    #[napi]
    pub fn get_analog_action(action_name: String) -> BigInt {
        let client = crate::client::get_client();
        BigInt::from(client.input().get_analog_action_handle(&action_name))
    }

    #[napi]
    pub fn shutdown() {
        let client = crate::client::get_client();
        client.input().shutdown()
    }
}
