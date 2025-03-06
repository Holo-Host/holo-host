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
