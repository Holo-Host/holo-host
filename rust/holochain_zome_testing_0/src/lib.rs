// use the re-exports from the integrity zome to ensure same versions
// use holochain_zome_testing_0_integrity::hdi;
use holochain_zome_testing_0_integrity::hdk;

use hdk::prelude::*;

#[hdk_extern]
fn roundtrip(input: String) -> Result<String, WasmError> {
    Ok(format!("roundtripping {input}"))
}
