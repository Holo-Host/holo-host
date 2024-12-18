use crate::inventory::InventoryBusType;
/// This module is used internally as a wrapper for pulling values from files under /sys. As with
/// anything else in the inventiry module, this is generally best-effort. We can't fail something
/// here and bubble it all the way up with '?' and have it not handled.
use log::info;
use std::fs;

pub fn string_attr(filename: String) -> Option<String> {
    // Not all devices are guaranteed to have all of the attributes. We need to consume any errors
    // and continue. It's better to get partial
    let ret = match fs::read_to_string(filename.clone()) {
        Ok(v) => v.strip_suffix("\n").unwrap_or_default().to_string(),
        Err(e) => {
            info!("Failed to read {} to a string: {}", &filename, e);
            return None;
        }
    };

    Some(ret)
}

pub fn integer_attr(filename: String) -> Option<u64> {
    if let Some(ret) = string_attr(filename.clone()) {
        let num_ret: Option<u64> = match ret.parse() {
            Ok(v) => Some(v),
            Err(e) => {
                info!(
                    "Failed to convert {} contents ({}) to int: {}",
                    &filename, ret, e
                );
                None
            }
        };
        return num_ret;
    }
    None
}

// Given a full device link path, return a path within the system's hardware device tree. This is
// generally always going to be the path without the `/sys/devices/` path prefix.
pub fn path_by_device_link(filename: &str) -> String {
    let mut ret = fs::canonicalize(filename)
        .unwrap()
        .to_string_lossy()
        .to_string();
    ret = ret.replace("/sys/devices/", "");
    ret
}

pub fn bus_by_device_link(filename: &str) -> InventoryBusType {
    let path = fs::canonicalize(filename)
        .unwrap()
        .to_string_lossy()
        .to_string();
    let fields = path.rsplit("/");
    for field in fields {
        // PCI or PCIe
        if field.starts_with("pci") {
            return InventoryBusType::PCI;
        }

        if field.starts_with("usb") {
            return InventoryBusType::USB;
        }

        if field.starts_with("ata") {
            return InventoryBusType::SATA;
        }
    }

    InventoryBusType::UNKNOWN
}
