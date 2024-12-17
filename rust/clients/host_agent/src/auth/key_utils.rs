// use std::sync::mpsc::{self, Receiver, Sender};
// use std::thread;
// use std::time::Duration;

// fn obtain_authorization_token() -> String {
//     // whatever you want, 3rd party token/username&password
//     String::new()
// }

// fn is_token_authorized(token: &str) -> bool {
//     // whatever logic to determine if the input authorizes the requester to obtain a user jwt
//     token.is_empty()
// }

// // request struct to exchange data
// struct UserRequest {
//     user_jwt_response_chan: Sender<String>,
//     user_public_key: String,
//     auth_info: String,
// }

// fn start_user_provisioning_service(is_authorized_cb: fn(&str) -> bool) -> Receiver<UserRequest> {
//     let (user_request_chan, user_request_receiver): (Sender<UserRequest>, Receiver<UserRequest>) =
//         mpsc::channel();

//     thread::spawn(move || {
//         let account_signing_key = get_account_signing_key(); // Setup, obtain account signing key
//         loop {
//             if let Ok(req) = user_request_receiver.recv() {
//                 // receive request
//                 if !is_authorized_cb(&req.auth_info) {
//                     println!("Request is not authorized to receive a JWT, timeout on purpose");
//                 } else if let Some(user_jwt) =
//                     generate_user_jwt(&req.user_public_key, &account_signing_key)
//                 {
//                     let _ = req.user_jwt_response_chan.send(user_jwt); // respond with jwt
//                 }
//             }
//         }
//     });

//     user_request_chan
// }

// fn start_user_process(
//     user_request_chan: Receiver<UserRequest>,
//     obtain_authorization_cb: fn() -> String,
// ) {
//     let request_user = |user_request_chan: Receiver<UserRequest>, auth_info: String| {
//         let (resp_chan, resp_receiver): (Sender<String>, Receiver<String>) = mpsc::channel();
//         let (user_public_key, _, user_key_pair) = generate_user_key();

//         // request jwt
//         let _ = user_request_chan.send(UserRequest {
//             user_jwt_response_chan: resp_chan,
//             user_public_key,
//             auth_info,
//         });

//         let user_jwt = resp_receiver.recv().unwrap(); // wait for response
//                                                       // user_jwt and user_key_pair can be used in conjunction with this nats.Option
//         let jwt_auth_option = nats::UserJWT::new(
//             move || Ok(user_jwt.clone()),
//             move |bytes| user_key_pair.sign(bytes),
//         );

//         // Alternatively you can create a creds file and use it as nats.Option
//         jwt_auth_option
//     };

//     thread::spawn(move || {
//         let jwt_auth_option = request_user(user_request_chan, obtain_authorization_cb());
//         let nc = nats::connect("nats://localhost:4111", jwt_auth_option).unwrap();
//         // simulate work one would want to do
//         thread::sleep(Duration::from_secs(1));
//     });
// }

// fn request_user_distributed() {
//     let req_chan = start_user_provisioning_service(is_token_authorized);
//     // start multiple user processes
//     for _ in 0..4 {
//         start_user_process(req_chan.clone(), obtain_authorization_token);
//     }
//     thread::sleep(Duration::from_secs(5));
// }

// // Placeholder functions for the missing implementations
// fn get_account_signing_key() -> String {
//     // Implementation here
//     String::new()
// }

// fn generate_user_jwt(user_public_key: &str, account_signing_key: &str) -> Option<String> {
//     // Implementation here
//     Some(String::new())
// }

// fn generate_user_key() -> (String, String, UserKeyPair) {
//     // Implementation here
//     (String::new(), String::new(), UserKeyPair {})
// }

// struct UserKeyPair;

// impl UserKeyPair {
//     fn sign(&self, _bytes: &[u8]) -> Result<Vec<u8>, ()> {
//         // Implementation here
//         Ok(vec![])
//     }
// }
