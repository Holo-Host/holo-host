// use the re-exports from the integrity zome to ensure same versions
// use holochain_zome_testing_0_integrity::hdi;
use holochain_zome_testing_0_integrity::{
    get_participant_registration_anchor_hash, hdk, types::LinkTypes,
};

use hdk::prelude::*;

#[hdk_extern]
pub fn init() -> ExternResult<InitCallbackResult> {
    let participant_registration_anchor_hash = get_participant_registration_anchor_hash()?;
    let AgentInfo {
        agent_latest_pubkey: my_pubkey,
        ..
    } = agent_info()?;

    create_link(
        participant_registration_anchor_hash,
        my_pubkey,
        LinkTypes::ParticipantRegistration,
        (),
    )?;

    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
pub fn get_registrations_pretty(_: ()) -> ExternResult<Vec<String>> {
    hdk::prelude::info!("get_registrations called info");
    hdk::prelude::debug!("get_registrations called debug");

    let participant_registration_anchor_hash = get_participant_registration_anchor_hash()?;

    let registrations = get_links(
        GetLinksInputBuilder::try_new(
            participant_registration_anchor_hash,
            LinkTypes::ParticipantRegistration,
        )?
        .build(),
    )?;

    let registrations_pretty = registrations
        .into_iter()
        .map(|link| format!("{link:#?}"))
        .collect();

    Ok(registrations_pretty)
}
