pub use hdi;
pub use hdk;

use hdi::prelude::*;

#[hdk_extern]
fn genesis_self_check(_: GenesisSelfCheckData) -> ExternResult<ValidateCallbackResult> {
    Ok(ValidateCallbackResult::Valid)
}

#[hdk_extern]
fn validate(op: Op) -> ExternResult<ValidateCallbackResult> {
    println!("validation op received: {op:?}");

    Ok(ValidateCallbackResult::Valid)
}

pub mod types {
    use hdi::prelude::*;

    #[hdk_link_types]
    pub enum LinkTypes {
        ParticipantRegistration,
    }
}

pub fn get_participant_registration_anchor_hash() -> ExternResult<EntryHash> {
    Path(vec!["_participants_".into()]).path_entry_hash()
}
