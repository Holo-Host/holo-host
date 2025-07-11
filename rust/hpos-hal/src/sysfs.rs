use crate::inventory::{HoloMemoryInventory, InventoryBusType};
/// This module is used internally as a wrapper for pulling values from files under /sys. As with
/// anything else in the inventiry module, this is generally best-effort. We can't fail something
/// here and bubble it all the way up with '?' and have it not handled.
use binrw::*;
use log::{debug, info};
use std::fmt::{self, Display};
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

/// See the SMBIOS spec, table 74 (Memory device structure for type 17). We really only use a few
/// fields from this structure to identify key device attributes.
#[derive(BinRead, Debug, PartialEq)]
#[br(little)]
pub struct SMBiosMemoryInfo {
    // dev_type is the SMBIOS type and should always be 17. Assert that as a sanity check.
    //dev_type: u8,
    #[brw(magic = 17u8)]
    // The data structure length will be at least 0x28 bytes in length if it's a data structure of
    // version 2.8 or later. The holoport SMBIOS version is 3.0.0. We ought to not have to deal
    // with anything older.
    #[br(assert(struct_len > 0x27))]
    struct_len: u8,
    handle: u16,
    phys_handle: u16,
    err_handle: u16,
    total_width: u16,
    data_width: u16,
    size: u16,
    form_factor: MemoryFormFactor,
    device_set: u8,
    // the follow values are indexes within the string table. Once we add code to parse the string
    // table part of the struct, these will point to the item in the string table vector/slice.
    device_locator_string_index: u8,
    bank_locator_string_index: u8,
    memory_type: MemoryType,
    // This is a bitmask
    type_detail: u16,
    speed_mts: u16,
    manufacturer_string_index: u8,
    serial_string_index: u8,
    asset_string_index: u8,
    part_string_index: u8,
    attributes: u8,
    extended_size: u16,
    configured_speed_mts: u16,
    minimum_voltage: u16,
    maximum_voltage: u16,
    configured_voltage: u16,
    memory_tech: u8,
    memory_modes: u16,
    firmware_version_index: u8,
    manufacturer_id: u16,
    product_id: u16,
    sub_manufacturer_id: u16,
    sub_product_id: u16,
    nonvolatile_size: u32,
    volatile_size: u32,
    cache_size: u32,
    logical_size: u32,
}

impl SMBiosMemoryInfo {
    /// This determines the capacity of the memory module in bytes using the magic incantation
    /// carved in runes in table 74 of the SMBIOS spec version 3.2.
    fn mem_size(&self) -> u64 {
        if self.size == 0x7fff {
            // 7.18.5 -- extended size
            self.extended_size as u64 * 1024_u64 * 1024_u64
        } else if self.size == 0xffff {
            // undetermined size
            0
        } else if (self.size & 0x8000) > 0 {
            // Size is in kilobytes
            self.size as u64 * 1024_u64
        } else {
            // size is in megabytes
            self.size as u64 * 1024_u64 * 1024_u64
        }
    }
}

// Table 7.18.1 - Memory device form factor from SMBIOS spec
#[derive(BinRead, Debug, PartialEq)]
pub enum MemoryFormFactor {
    #[br(magic = 0u8)]
    Invalid,
    #[br(magic = 1u8)]
    Other,
    #[br(magic = 2u8)]
    Unknown,
    #[br(magic = 3u8)]
    SIMM,
    #[br(magic = 4u8)]
    SIP,
    // Who doesn't like chips...
    #[br(magic = 5u8)]
    Chip,
    // ... and dip?
    #[br(magic = 6u8)]
    DIP,
    #[br(magic = 7u8)]
    ZIP,
    #[br(magic = 8u8)]
    Proprietary,
    #[br(magic = 9u8)]
    DIMM,
    #[br(magic = 10u8)]
    TSOP,
    #[br(magic = 11u8)]
    ChipRow,
    #[br(magic = 12u8)]
    RIMM,
    // Most common for hardware we'll be dealing with.
    #[br(magic = 13u8)]
    SODIMM,
    #[br(magic = 14u8)]
    SRIMM,
    #[br(magic = 15u8)]
    FBDIMM,
}

impl Display for MemoryFormFactor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "Invalid"),
            Self::Other => write!(f, "Other"),
            Self::Unknown => write!(f, "Unknown"),
            Self::SIMM => write!(f, "SIMM"),
            Self::SIP => write!(f, "SIP"),
            Self::Chip => write!(f, "Chip"),
            Self::DIP => write!(f, "DIP"),
            Self::ZIP => write!(f, "ZIP"),
            Self::Proprietary => write!(f, "Proprietary"),
            Self::DIMM => write!(f, "DIMM"),
            Self::TSOP => write!(f, "TSOP"),
            Self::ChipRow => write!(f, "Row of Chips"),
            Self::RIMM => write!(f, "RIMM"),
            Self::SODIMM => write!(f, "SODIMM"),
            Self::SRIMM => write!(f, "SRIMM"),
            Self::FBDIMM => write!(f, "FBDIMM"),
        }
    }
}

// Table 7.18.2 - Memory device type from SMBIOS spec
#[derive(BinRead, Debug, PartialEq)]
pub enum MemoryType {
    #[br(magic = 0u8)]
    Invalid,
    #[br(magic = 1u8)]
    Other,
    #[br(magic = 2u8)]
    Unknown,
    #[br(magic = 3u8)]
    DRAM,
    #[br(magic = 4u8)]
    EDRAM,
    #[br(magic = 5u8)]
    VRAM,
    #[br(magic = 6u8)]
    SRAM,
    #[br(magic = 7u8)]
    RAM,
    #[br(magic = 8u8)]
    ROM,
    #[br(magic = 9u8)]
    FLASH,
    #[br(magic = 10u8)]
    EEPROM,
    #[br(magic = 11u8)]
    FEPROM,
    #[br(magic = 12u8)]
    EPROM,
    #[br(magic = 13u8)]
    CDRAM,
    #[br(magic = 14u8)]
    THREEDRAM,
    #[br(magic = 15u8)]
    SDRAM,
    #[br(magic = 16u8)]
    SGRAM,
    #[br(magic = 17u8)]
    RDRAM,
    #[br(magic = 18u8)]
    DDR,
    #[br(magic = 19u8)]
    DDR2,
    #[br(magic = 20u8)]
    DDR2FBDIMM,
    #[br(magic = 21u8)]
    Reserved1,
    #[br(magic = 22u8)]
    Reserved2,
    #[br(magic = 23u8)]
    Reserved3,
    #[br(magic = 24u8)]
    DDR3,
    #[br(magic = 25u8)]
    FBD2,
    #[br(magic = 26u8)]
    DDR4,
    #[br(magic = 27u8)]
    LPDDR,
    #[br(magic = 28u8)]
    LPDDR2,
    #[br(magic = 29u8)]
    LPDDR3,
    #[br(magic = 30u8)]
    LPDDR4,
    #[br(magic = 31u8)]
    LNVRAM,
}

impl Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "Invalid"),
            Self::Other => write!(f, "Other"),
            Self::Unknown => write!(f, "Unknown"),
            Self::DRAM => write!(f, "DRAM"),
            Self::EDRAM => write!(f, "EDRAM"),
            Self::RAM => write!(f, "RAM"),
            Self::VRAM => write!(f, "VRAM"),
            Self::SRAM => write!(f, "SRAM"),
            Self::ROM => write!(f, "ROM"),
            Self::FLASH => write!(f, "FLASH"),
            Self::EEPROM => write!(f, "EEPROM"),
            Self::FEPROM => write!(f, "FEPROM"),
            Self::EPROM => write!(f, "EPROM"),
            Self::CDRAM => write!(f, "CDRAM"),
            Self::THREEDRAM => write!(f, "THREEDRAM"),
            Self::SDRAM => write!(f, "SDRAM"),
            Self::SGRAM => write!(f, "SGRAM"),
            Self::RDRAM => write!(f, "RDRAM"),
            Self::DDR => write!(f, "DDR"),
            Self::DDR2 => write!(f, "DDR2"),
            Self::DDR2FBDIMM => write!(f, "DDR2FBDIMM"),
            Self::Reserved1 => write!(f, "Reserved1"),
            Self::Reserved2 => write!(f, "Reserved2"),
            Self::Reserved3 => write!(f, "Reserved3"),
            Self::DDR3 => write!(f, "DDR3"),
            Self::FBD2 => write!(f, "FBD2"),
            Self::DDR4 => write!(f, "DDR4"),
            Self::LPDDR => write!(f, "LPDDR"),
            Self::LPDDR2 => write!(f, "LPDDR2"),
            Self::LPDDR3 => write!(f, "LPDDR3"),
            Self::LPDDR4 => write!(f, "LPDDR4"),
            Self::LNVRAM => write!(f, "LNVRAM"),
        }
    }
}

/// Parse an SMBIOS type 17 struct. There's a lot of other useful information in here, including
/// things like memory speed and manufacturer and model strings. Those will be useful to add later,
/// but for now, having each memory "slot" and the capacity in it will be super useful. Note that a
/// "slot" no longer equates to a physical memory slot on the board, and in many hardware cases,
/// there are no slots at all and the memory is soldered to the board...
pub fn parse_mem_file(path: &str) -> Result<crate::inventory::HoloMemoryInventory, Error> {
    let mut f = std::fs::File::open(path)?;
    let mem = SMBiosMemoryInfo::read(&mut f)?;
    debug!("Parsed memory structure: {:?}", mem);
    Ok(HoloMemoryInventory {
        size: mem.mem_size(),
        form_factor: mem.form_factor.to_string(),
        memory_type: mem.memory_type.to_string(),
        memory_speed: mem.speed_mts,
    })
}
