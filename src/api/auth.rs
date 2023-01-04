use napi_derive::napi;

#[napi]
pub mod auth {
    use napi::bindgen_prelude::{Error, Uint8Array};
    use steamworks::{AuthSessionTicketResponse, AuthTicket};
    use tokio::sync::oneshot;

    #[napi]
    pub struct Ticket {
        pub(crate) data: Vec<u8>,
        pub(crate) handle: AuthTicket,
    }

    #[napi]
    impl Ticket {
        #[napi]
        pub fn cancel(&mut self) {
            let client = crate::client::get_client();
            client.user().cancel_authentication_ticket(self.handle);
        }

        #[napi]
        pub fn get_bytes(&self) -> Uint8Array {
            self.data.clone().into()
        }
    }

    /// @param timeoutSeconds - The number of seconds to wait for the ticket to be validated. Default value is 10 seconds.
    #[napi]
    pub async fn get_session_ticket(timeout_seconds: Option<u32>) -> Result<Ticket, Error> {
        let client = crate::client::get_client();
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);

        let (ticket_handle, ticket) = client.user().authentication_session_ticket();
        let callback =
            client.register_callback(move |session_ticket_response: AuthSessionTicketResponse| {
                if session_ticket_response.ticket == ticket_handle {
                    if let Some(tx) = tx.take() {
                        tx.send(match session_ticket_response.result {
                            Ok(()) => Ok(()),
                            Err(e) => Err(Error::from_reason(e.to_string())),
                        })
                        .unwrap();
                    }
                }
            });

        let mut ticket = Ticket {
            data: ticket,
            handle: ticket_handle,
        };

        let timeout_seconds = u64::from(timeout_seconds.unwrap_or(10));
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(timeout_seconds), rx).await;

        drop(callback);

        match result {
            Ok(result) => match result.unwrap() {
                Ok(()) => Ok(ticket),
                Err(e) => Err(Error::from_reason(e.to_string())),
            },
            Err(_) => {
                ticket.cancel();
                return Err(Error::from_reason(
                    "Steam didn't validated the ticket in time.",
                ));
            }
        }
    }
}
