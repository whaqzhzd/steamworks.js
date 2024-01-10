use sys::InputHandle_t;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum XSteamInputType {
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

/// Access to the steam input interface
pub struct Input<Manager> {
    pub(crate) input: *mut sys::ISteamInput,
    pub(crate) _inner: Arc<Inner<Manager>>,
}

impl<Manager> Input<Manager> {
    /// Init must be called when starting use of this interface.
    /// if explicitly_call_run_frame is called then you will need to manually call RunFrame
    /// each frame, otherwise Steam Input will updated when SteamAPI_RunCallbacks() is called
    pub fn init(&self, explicitly_call_run_frame: bool) {
        unsafe {
            sys::SteamAPI_ISteamInput_Init(self.input, explicitly_call_run_frame);
        }
    }

    /// Synchronize API state with the latest Steam Input action data available. This
    /// is performed automatically by SteamAPI_RunCallbacks, but for the absolute lowest
    /// possible latency, you call this directly before reading controller state.
    /// Note: This must be called from somewhere before GetConnectedControllers will
    /// return any handles
    pub fn run_frame(&self) {
        unsafe { sys::SteamAPI_ISteamInput_RunFrame(self.input, false) }
    }

    /// Returns a list of the currently connected controllers
    pub fn get_connected_controllers(&self) -> Vec<sys::InputHandle_t> {
        unsafe {
            let handles = [0_u64; sys::STEAM_INPUT_MAX_COUNT as usize].as_mut_ptr();
            let quantity = sys::SteamAPI_ISteamInput_GetConnectedControllers(self.input, handles);
            if quantity == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(handles as *const _, quantity as usize).to_vec()
            }
        }
    }

    pub fn get_controller_type(&self, handle: u64) -> XSteamInputType {
        unsafe {
            match sys::SteamAPI_ISteamInput_GetInputTypeForHandle(self.input, handle) {
                sys::ESteamInputType::k_ESteamInputType_Unknown => {
                    XSteamInputType::KEsteamInputTypeUnknown
                }
                sys::ESteamInputType::k_ESteamInputType_SteamController => {
                    XSteamInputType::KEsteamInputTypeSteamController
                }
                sys::ESteamInputType::k_ESteamInputType_XBox360Controller => {
                    XSteamInputType::KEsteamInputTypeXbox360controller
                }
                sys::ESteamInputType::k_ESteamInputType_XBoxOneController => {
                    XSteamInputType::KEsteamInputTypeXboxOneController
                }
                sys::ESteamInputType::k_ESteamInputType_GenericGamepad => {
                    XSteamInputType::KEsteamInputTypeGenericGamepad
                }
                sys::ESteamInputType::k_ESteamInputType_PS4Controller => {
                    XSteamInputType::KEsteamInputTypePs4controller
                }
                sys::ESteamInputType::k_ESteamInputType_AppleMFiController => {
                    XSteamInputType::KEsteamInputTypeAppleMfiController
                }
                sys::ESteamInputType::k_ESteamInputType_AndroidController => {
                    XSteamInputType::KEsteamInputTypeAndroidController
                }
                sys::ESteamInputType::k_ESteamInputType_SwitchJoyConPair => {
                    XSteamInputType::KEsteamInputTypeSwitchJoyConPair
                }
                sys::ESteamInputType::k_ESteamInputType_SwitchJoyConSingle => {
                    XSteamInputType::KEsteamInputTypeSwitchJoyConSingle
                }
                sys::ESteamInputType::k_ESteamInputType_SwitchProController => {
                    XSteamInputType::KEsteamInputTypeSwitchProController
                }
                sys::ESteamInputType::k_ESteamInputType_MobileTouch => {
                    XSteamInputType::KEsteamInputTypeMobileTouch
                }
                sys::ESteamInputType::k_ESteamInputType_PS3Controller => {
                    XSteamInputType::KEsteamInputTypePs3controller
                }
                sys::ESteamInputType::k_ESteamInputType_PS5Controller => {
                    XSteamInputType::KEsteamInputTypePs5controller
                }
                sys::ESteamInputType::k_ESteamInputType_SteamDeckController => {
                    XSteamInputType::KEsteamInputTypeSteamDeckController
                }
                sys::ESteamInputType::k_ESteamInputType_Count => {
                    XSteamInputType::KEsteamInputTypeCount
                }
                sys::ESteamInputType::k_ESteamInputType_MaximumPossibleValue => {
                    XSteamInputType::KEsteamInputTypeMaximumPossibleValue
                }
                _ => XSteamInputType::KEsteamInputTypeUnknown,
            }
        }
    }

    /// Returns a list of the currently connected controllers without allocating, and the count
    pub fn get_connected_controllers_slice(
        &self,
        mut controllers: impl AsMut<[InputHandle_t]>,
    ) -> usize {
        let handles = controllers.as_mut();
        assert!(handles.len() >= sys::STEAM_INPUT_MAX_COUNT as usize);
        unsafe {
            return sys::SteamAPI_ISteamInput_GetConnectedControllers(
                self.input,
                handles.as_mut_ptr(),
            ) as usize;
        }
    }

    /// Returns the associated ControllerActionSet handle for the specified controller,
    pub fn get_action_set_handle(&self, action_set_name: &str) -> sys::InputActionSetHandle_t {
        let name = CString::new(action_set_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetActionSetHandle(self.input, name.as_ptr()) }
    }

    /// Reconfigure the controller to use the specified action set
    /// This is cheap, and can be safely called repeatedly.
    pub fn activate_action_set_handle(
        &self,
        input_handle: sys::InputHandle_t,
        action_set_handle: sys::InputActionSetHandle_t,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_ActivateActionSet(self.input, input_handle, action_set_handle)
        }
    }

    /// Get the handle of the specified Digital action.
    pub fn get_digital_action_handle(&self, action_name: &str) -> sys::InputDigitalActionHandle_t {
        let name = CString::new(action_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetDigitalActionHandle(self.input, name.as_ptr()) }
    }

    /// Get the handle of the specified Analog action.
    pub fn get_analog_action_handle(&self, action_name: &str) -> sys::InputAnalogActionHandle_t {
        let name = CString::new(action_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetAnalogActionHandle(self.input, name.as_ptr()) }
    }

    /// Returns the current state of the supplied digital game action.
    pub fn get_digital_action_data(
        &self,
        input_handle: sys::InputHandle_t,
        action_handle: sys::InputDigitalActionHandle_t,
    ) -> sys::InputDigitalActionData_t {
        unsafe {
            sys::SteamAPI_ISteamInput_GetDigitalActionData(self.input, input_handle, action_handle)
        }
    }

    /// Returns the current state of the supplied analog game action.
    pub fn get_analog_action_data(
        &self,
        input_handle: sys::InputHandle_t,
        action_handle: sys::InputAnalogActionHandle_t,
    ) -> sys::InputAnalogActionData_t {
        unsafe {
            sys::SteamAPI_ISteamInput_GetAnalogActionData(self.input, input_handle, action_handle)
        }
    }

    pub fn get_motion_data(&self, input_handle: sys::InputHandle_t) -> sys::InputMotionData_t {
        unsafe { sys::SteamAPI_ISteamInput_GetMotionData(self.input, input_handle) }
    }

    /// Shutdown must be called when ending use of this interface.
    pub fn shutdown(&self) {
        unsafe {
            sys::SteamAPI_ISteamInput_Shutdown(self.input);
        }
    }
}
