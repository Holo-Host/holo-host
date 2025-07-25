use crate::fs::parse_fs;
use crate::sysfs;
/// Module for handling inventory collection, including hardware, platform, system level attributes
/// and components. This shouldn't ever fail and is considered best-effort. The intent is that the
/// caller gathers inventory periodically, and the caller compares it with previous copies of the
/// inventory, highlighting differences. To facilitate this, many of the operations throughout will
/// return empty data and swallow and errors, rather than abort and return no inventory.
use glob::glob;
use log::{debug, info};
use procfs::{CpuInfo, FromBufRead};
use serde_derive::{Deserialize, Serialize};
use std::fmt::{self, Display};
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::path::Path;
// Using fork/exec unnecessarily will make the inventory collection too expensive to run as often
// as we are going to need to. It is almost never necessary.
use std::process::Command;
use std::{fs, fs::File};
use thiserror::Error;
use thiserror_context::{impl_context, Context};

/// A consistent wrapper around all of the types of errors we might get from various subsystems.
#[derive(Debug, Error)]
pub enum InventoryErrorInner {
    #[error("I/O Error")]
    InputOutput(#[from] io::Error),
    #[error("Parse Error")]
    Parse(#[from] binrw::Error),
    #[error("UTF8 Conversion Error")]
    UTF8(#[from] std::str::Utf8Error),
    #[error("Object not found")]
    NotFound,
    #[error("Serialisation Error")]
    SerialisationError(#[from] serde_json::Error),
}
impl_context!(InventoryError(InventoryErrorInner));

/// A data structure representing a host's inventory, including hardware, firmware, and
/// infrequently-changing and infrequently-changing software attributes.
///
/// Retrieving an inventory data structure from the current host, the following ought to work:
///
/// ```rust
/// use hpos_hal::inventory::HoloInventory;
///
/// let inv = HoloInventory::from_host();
/// ```
///
/// This must be performed as the root user, as it is necessary to read some privileged information
/// from the kernel.
///
/// An example that agent might use when a user, or support, runs a command such as `holo-agent
/// model` (or similar) might be:
///
/// ```rust
/// use hpos_hal::inventory::HoloInventory;
///
/// let inv = HoloInventory::from_host();
///
/// println!("Hardware Model: {}", inv.platform.unwrap().platform_type.to_string());
/// ````
///
/// This data structure can also be serialized and deserialized via serde_derive;
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloInventory {
    /// Data structure representing a number of system-wide attributes, including kernel version
    /// and systemd machine ID.
    pub system: HoloSystemInventory,
    /// Data structures representing physical drives and their attributes. This also includes
    /// partitions contained within the device, and some filesystems within those partitions.
    pub drives: Vec<HoloDriveInventory>,
    /// Information about physical NICs present. This refers to the hardware devices, which is not
    /// necessarily the same as the network interfaces managed by tools like `ip`.
    pub nics: Vec<HoloNicInventory>,
    /// Information about CPUs present. All CPUs are generally the same in the x86_64 case, but may
    /// be different on other architectures, such as aarch64.
    pub cpus: Vec<HoloProcessorInventory>,
    /// Inventory of memory DIMM devices
    pub mem: Vec<HoloMemoryInventory>,
    /// An inventory of USB devices specifically. May overlap with other sections (eg, USB storage
    /// devices).
    pub usb: Vec<HoloUsbInventory>,
    /// Generally x86-specific SMBIOS/DMI information provided by the hardware vendor.
    pub smbios: HoloSMBIOS,
    /// An overall categorisation of this host as a platform. This might include guesses at the
    /// model of hardware, or the hypervisor we're running on.
    pub platform: Option<HoloPlatform>,
}

impl HoloInventory {
    /// Saves the HoloInventory struct to a file in JSON format.
    pub fn save_to_file(&self, path: &str) -> Result<(), InventoryError> {
        log::trace!("Saving inventory to file: path={path:?}");

        // Ensure directory exists
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .context("ERROR: Failed to create host inventory path directory.")?;
            }
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(InventoryError::from)?;

        serde_json::to_writer_pretty(file, self).map_err(InventoryError::from)?;

        Ok(())
    }

    /// Reads the HoloInventory struct from a file and deserializes it.
    pub fn load_from_file(path: &str) -> Result<Self, InventoryError> {
        let mut file = File::open(path).map_err(InventoryError::from)?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(InventoryError::from)?;

        let inventory = serde_json::from_str(&content).map_err(InventoryError::from)?;
        Ok(inventory)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloSystemInventory {
    /// The FreeDesktop.org systemd machine ID that uniquely identifies this installed instance of
    /// systemd.
    pub machine_id: String,
    /// The Linux kernel build string.
    pub kernel_version: String,
    /// OpenSSH Host public keys.
    pub ssh_host_keys: Vec<SSHPubKey>,
    /// Distribution release string
    pub distribution_release: String,
    /// Package manager version/release string
    pub package_mgr_release: String,
}

/// A data structure representing an OpenSSH public key. When stored, each key is a single line of
/// text in a single file, consisting of three fields separated by spaces. The key tyoe, the key
/// matter itself, and an optional label for the key. This data structure parses the fields out
/// separately, but these keys can be reassembled for use with OpenSSH and other tools.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct SSHPubKey {
    /// The key type, for example `ecdsa-sha2-nistp256`. See OpenSSH's `ssh-keygen(1)` man page for
    /// options.
    pub key_type: String,
    /// The encoded public key data.
    pub key: String,
    /// An optional label. Must be present, even if it's an empty string.
    pub label: String,
}

/// Data structure containing any SMBIOS/DMI attributes and identifiers that might be present.
/// These are generally useful for identifying components within a node when provided by the
/// vendor. Most vendors provide this info, holoport hardware currently doesn't provide anything
/// useful in these fields, most hypervisors allow these to be set as part of the attributes of the
/// virtual machine (libvirt, for example can set these for KVM and Xen VMs). As a result, some
/// cloud providers also fill these in with useful attributes.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloSMBIOS {
    /// Date of BIOS release
    pub bios_date: Option<String>,
    /// Version of VIOS release
    pub bios_release: Option<String>,
    /// Name of BIOS vendor
    pub bios_vendor: Option<String>,
    /// BIOS version string
    pub bios_version: Option<String>,
    /// System board asset tag
    pub board_asset_tag: Option<String>,
    /// System board model name
    pub board_name: Option<String>,
    /// System board serial number
    pub board_serial: Option<String>,
    /// System board vendor
    pub board_vendor: Option<String>,
    /// System board version
    pub board_version: Option<String>,
    /// Host chassis serial number
    pub chassis_serial: Option<String>,
    /// Host chassis vendor name
    pub chassis_vendor: Option<String>,
    /// Host chassis form factor hint (see SMBIOS reference for valid types)
    pub chassis_type: Option<String>,
    /// Version of chassis design/build
    pub chassis_version: Option<String>,
    /// Host product family string
    pub product_family: Option<String>,
    /// Host product model name
    pub product_name: Option<String>,
    /// Host producct serial number
    pub product_serial: Option<String>,
    /// Product SKU
    pub product_sku: Option<String>,
    /// Product UUID
    pub product_uuid: Option<String>,
    /// System vendor name
    pub sys_vendor: Option<String>,
}

/// A structure representing USB devices connected to a Holo Host.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloUsbInventory {
    /// USB device class
    class: Option<String>,
    /// USB device subclass
    subclass: Option<String>,
    /// USB Vendor ID
    vendor_id: Option<String>,
    /// USB Product ID
    product_id: Option<String>,
    /// USB version
    usb_version: Option<String>,
    /// Path along USB bus
    path: String,
}

impl HoloUsbInventory {
    const USB_DEV_GLOB: &str = "/sys/bus/usb/devices/*";
    const UNINTERESTING_CLASSES: [&str; 1] = ["09"];
    pub fn from_host() -> Vec<HoloUsbInventory> {
        let mut ret: Vec<HoloUsbInventory> = vec![];

        for usb_dev in glob(Self::USB_DEV_GLOB).unwrap() {
            // TODO: grab fill in fields if the class isn't a hub
            let usb_dev = usb_dev.unwrap().clone();
            let dev_base = format!("{}", usb_dev.to_string_lossy());
            let usb_path = fs::canonicalize(&dev_base).unwrap_or_default();
            let mut usb_path_clone = usb_path.clone();
            usb_path_clone.push("bDeviceClass");

            if std::fs::exists(usb_path_clone).unwrap_or(false) {
                let usb_path = usb_path.to_string_lossy();
                debug!("USB link: {}", &usb_path);

                let usb_class = sysfs::string_attr(format!("{}/bDeviceClass", dev_base));
                // We aren't interested in things like USB hubs. We should instead ignore those.
                match usb_class {
                    Some(class) => {
                        if Self::UNINTERESTING_CLASSES.contains(&class.as_str()) {
                            continue;
                        }

                        // This device is something of potential interest
                        let vendor_id = sysfs::string_attr(format!("{}/idVendor", dev_base));
                        let product_id = sysfs::string_attr(format!("{}/idProduct", dev_base));
                        let subclass = sysfs::string_attr(format!("{}/bDeviceSubClass", dev_base));
                        let usb_version = sysfs::string_attr(format!("{}/version", dev_base));

                        // Add to inventory
                        debug!("Adding USB device {}", usb_path);
                        ret.push(HoloUsbInventory {
                            class: Some(class),
                            subclass,
                            vendor_id,
                            product_id,
                            usb_version,
                            path: usb_path.to_string(),
                        });
                    }
                    None => continue,
                }
            }
        }

        ret
    }
}

/// A struct to represent a discovered LED device
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum HoloLedDevice {
    HoloportUsbLed { device_node: String },
    None(),
}
impl Default for HoloLedDevice {
    fn default() -> Self {
        HoloLedDevice::None()
    }
}

/// A structure representing Holo Platform related meta-inventory
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct HoloPlatform {
    pub platform_type: HoloPlatformType,
    pub hypervisor_guest: bool,
    pub admin_interface: Option<String>,
    pub system_drive: Option<String>,
    pub data_drive: Option<String>,
    pub led_device: HoloLedDevice,
}

impl Display for HoloPlatform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Platform Model: {}\nRunning on Hypervisor: {}",
            self.platform_type, self.hypervisor_guest
        )
    }
}
// TODO: This needs more work and testing against real hardware.
impl HoloPlatform {
    /// Given an inventory structure, use some heuristics to give a best-guess at the type of
    /// platform this is.
    pub fn from_inventory(inventory: &HoloInventory) -> Self {
        // In this part of the inventory, we're using a variety of heuristics to determine high
        // level constructs that we would like to treat as real and tangible (such as a holoport
        // model or whether it's a VM or not), and make it consistent across the code running on
        // hosts (agent, Nix*), as well as the centralised services managing the whole armada of
        // machines.
        let platform_type = Self::guess_platform(inventory);
        let hypervisor_guest = Self::guess_hypervisor(inventory);

        // This is manageable while we have a very limited number of platform models to support,
        // but could become unwieldy in future. At very least, it could become a separate module,
        // or perhaps something more dynamic such as being defined in a JSON file.
        match platform_type {
            // TODO: find admin interface and system drive by path.
            HoloPlatformType::Holoport => Self {
                platform_type,
                hypervisor_guest,
                admin_interface: None,
                system_drive: None,
                data_drive: None,
                led_device: HoloLedDevice::HoloportUsbLed {
                    device_node: "ttyUSB0".to_string(),
                },
            },
            HoloPlatformType::HoloportNoLed => Self {
                platform_type,
                hypervisor_guest,
                admin_interface: None,
                system_drive: None,
                data_drive: None,
                led_device: HoloLedDevice::None(),
            },
            // TODO: find admin interface and system and data drives by path.
            HoloPlatformType::HoloportPlus => Self {
                platform_type,
                hypervisor_guest,
                admin_interface: None,
                system_drive: None,
                data_drive: None,
                led_device: HoloLedDevice::HoloportUsbLed {
                    device_node: "ttyUSB0".to_string(),
                },
            },
            HoloPlatformType::HoloportPlusNoLed => Self {
                platform_type,
                hypervisor_guest,
                admin_interface: None,
                system_drive: None,
                data_drive: None,
                led_device: HoloLedDevice::None(),
            },
            HoloPlatformType::Yoloport => Self {
                platform_type,
                hypervisor_guest,
                admin_interface: None,
                system_drive: None,
                data_drive: None,
                led_device: HoloLedDevice::None(),
            },
            HoloPlatformType::Unknown => {
                // Unknown model type, so we don't have any hints for admin interface, etc. Note:
                // it may be better to just pick or guess at one, or have it able to be configured or
                // overridden, so that consumers can rely more heavily on it.
                Self {
                    platform_type,
                    hypervisor_guest,
                    admin_interface: None,
                    system_drive: None,
                    data_drive: None,
                    led_device: HoloLedDevice::None(),
                }
            }
        }
    }

    /// Guess whether the operating system thinks we're running as a VM under a hypervisor. Later
    /// we could choose to also guess which hypervisor or cloud provider, and potentially also read
    /// custom fields and OEM strings from the DMI pool data to give us better identifying
    /// information that will be useful to our management services.
    fn guess_hypervisor(inventory: &HoloInventory) -> bool {
        if !inventory.cpus.is_empty() {
            // Check to see whether the hypervisor flag is set for the first CPU
            if inventory.cpus[0].flags.contains(&"hypervisor".to_string()) {
                return true;
            }
        }
        false
    }

    // Guess whether this is a holoport node or not. This is currently incomplete. We'll need to
    // use additional heuristics to be more sure that this is a holoport. But this is a start.
    fn guess_platform(inventory: &HoloInventory) -> HoloPlatformType {
        // holoports have a single NIC of a specific model at a specific part of the PCI tree. We
        // should add more criteria for determining whether it's a holoport or not, but this is a
        // start.
        if inventory.nics.len() == 1
            && inventory.nics[0].location == "pci0000:00/0000:00:1c.0/0000:01:00.0"
            && inventory.nics[0].model == Some("0x8168".to_string())
            && inventory.nics[0].vendor == Some("0x10ec".to_string())
        {
            // The main difference between a Holoport and Holoport Plus is that a
            // Holoport has a single 1G SATA rotational drive model ST1000LM035-1RK1. A holoport
            // plus has a 2G SATA rotational drive model ST2000LM015-2E81 and a SATA SSD model
            // KINGSTON RBUSMS1.
            //
            // Note too that there are variants of these criteria that don't have a USB LED device.
            // The have an LED, but it's a simple on/off state. The [leds] module needs us to help
            // distinguish between the two.
            //
            // We may need to relax some of the criteria here, around the path. We're making an
            // assumption that the drives have been cabled consistently for each holoport, but
            // we've yet to see whether that's the case in the field.
            let has_usb_led: bool = true; // TODO: FIXME

            for drive in inventory.drives.iter() {
                if drive.location == "pci0000:00/0000:00:17.0/ata3/host2/target2:0:0/2:0:0:0"
                    && drive.model == Some("ST1000LM035-1RK1".to_string())
                {
                    if has_usb_led {
                        return HoloPlatformType::Holoport;
                    } else {
                        return HoloPlatformType::HoloportNoLed;
                    }
                }
            }
            for drive in inventory.drives.iter() {
                if drive.location == "pci0000:00/0000:00:17.0/ata2/host1/target1:0:0/1:0:0:0"
                    && drive.model == Some("ST2000LM015-2E81".to_string())
                {
                    // We could/should also check for the SSD here too, but this will work for
                    // now.
                    if has_usb_led {
                        return HoloPlatformType::HoloportPlus;
                    } else {
                        return HoloPlatformType::HoloportPlusNoLed;
                    }
                }
            }
        }

        // This model is a placeholder for testing in the short term and can be removed later.
        // It's a good example of how easy it can be with hardware that has its DMI attributes
        // filled in.
        if inventory.smbios.product_name == Some("XPS 13 9310".to_string()) {
            return HoloPlatformType::Yoloport;
        }

        // We tried...
        HoloPlatformType::Unknown
    }
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub enum HoloPlatformType {
    /// A Holoport node
    Holoport,
    /// A holoport with no USB LED
    HoloportNoLed,
    /// A Holodport Plus node
    HoloportPlus,
    /// A Holoport Plus with no USB LED
    HoloportPlusNoLed,
    /// Temporary model type just for testing in the short term
    Yoloport,
    /// Not known
    #[default]
    Unknown,
}

impl Display for HoloPlatformType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HoloPlatformType::Holoport => write!(f, "Holoport"),
            HoloPlatformType::HoloportNoLed => write!(f, "Holoport (no LED)"),
            HoloPlatformType::HoloportPlus => write!(f, "Holoport Plus"),
            HoloPlatformType::HoloportPlusNoLed => write!(f, "Holoport Plus (no LED)"),
            HoloPlatformType::Yoloport => write!(f, "YOLO port (testing)"),
            HoloPlatformType::Unknown => write!(f, "Unknown Platform Type"),
        }
    }
}

const DMI_ROOT: &str = "/sys/class/dmi/id";

impl HoloInventory {
    pub fn from_host() -> Self {
        let mut inv = HoloInventory {
            smbios: HoloSMBIOS {
                bios_date: sysfs::string_attr(format!("{}/bios_date", DMI_ROOT)),
                bios_release: sysfs::string_attr(format!("{}/bios_release", DMI_ROOT)),
                bios_vendor: sysfs::string_attr(format!("{}/bios_vendor", DMI_ROOT)),
                bios_version: sysfs::string_attr(format!("{}/bios_version", DMI_ROOT)),
                board_asset_tag: sysfs::string_attr(format!("{}/board_asset_tag", DMI_ROOT)),
                board_name: sysfs::string_attr(format!("{}/board_name", DMI_ROOT)),
                board_serial: sysfs::string_attr(format!("{}/board_serial", DMI_ROOT)),
                board_vendor: sysfs::string_attr(format!("{}/board_vendor", DMI_ROOT)),
                board_version: sysfs::string_attr(format!("{}/board_version", DMI_ROOT)),
                chassis_serial: sysfs::string_attr(format!("{}/chassis_serial", DMI_ROOT)),
                chassis_type: sysfs::string_attr(format!("{}/chassis_type", DMI_ROOT)),
                chassis_vendor: sysfs::string_attr(format!("{}/chassis_vendor", DMI_ROOT)),
                chassis_version: sysfs::string_attr(format!("{}/chassis_version", DMI_ROOT)),
                product_family: sysfs::string_attr(format!("{}/product_family", DMI_ROOT)),
                product_name: sysfs::string_attr(format!("{}/product_name", DMI_ROOT)),
                product_serial: sysfs::string_attr(format!("{}/product_serial", DMI_ROOT)),
                product_sku: sysfs::string_attr(format!("{}/product_sku", DMI_ROOT)),
                product_uuid: sysfs::string_attr(format!("{}/product_uuid", DMI_ROOT)),
                sys_vendor: sysfs::string_attr(format!("{}/sys_vendor", DMI_ROOT)),
            },
            system: HoloSystemInventory {
                machine_id: systemd_machine_id(),
                kernel_version: linux_kernel_build(),
                ssh_host_keys: ssh_host_keys(),
                distribution_release: get_distro_release(),
                package_mgr_release: get_package_mgr_release(),
            },
            drives: HoloDriveInventory::from_host(),
            cpus: HoloProcessorInventory::from_host(),
            mem: HoloMemoryInventory::from_host(),
            nics: HoloNicInventory::from_host(),
            usb: HoloUsbInventory::from_host(),
            platform: None,
        };

        let plat = HoloPlatform::from_inventory(&inv);
        inv.platform = Some(plat);

        inv
    }
}

/// Data structure representing physical drives, and the partitions within them. Virtual device,
/// such as loopback block devices, aren't tracked in this list. Only physical drives.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloDriveInventory {
    /// Block device node for drive
    pub block_dev: String,
    /// Drive serial string
    pub serial: Option<String>,
    /// Drive model string
    pub model: Option<String>,
    /// Drive unique world-wide identifier (WWID). May not be present for all drive types.
    pub wwid: Option<String>,
    /// Drive universally-unique identifier. May not be present for all drive types.
    pub uuid: Option<String>,
    /// Drive transport/bus type. For example, PCIe in the case of NVMe drives, or USB for USB
    /// drives.
    pub bus: InventoryBusType,
    /// A string representing the location of the drive in the hardware device tree.
    pub location: String,
    /// Drive capacity in bytes.
    pub capacity_bytes: Option<u64>,
    /// A list of partition objects, if present.
    pub partitions: Vec<HoloPartitionInventory>,
    /// Whole-device filesystem, if present
    pub filesystem: Option<HoloFilesystemInventory>,
}

/// Glob used to find block devices that are hardware-backed. This primarily consists of
/// whole-drive block device nodes representing a whole physical drive. THis doesn't include
/// partitions, or virtual block devices such as loopback block devices.
const BLOCK_DEV_SYSFS_GLOB: &str = "/sys/class/block/*/device";

impl HoloDriveInventory {
    /// Generates an inventory structure from the machine we're currently executing on.
    pub fn from_host() -> Vec<HoloDriveInventory> {
        let mut ret: Vec<HoloDriveInventory> = vec![];
        // Find all of the physical drives starting at the block device
        for phys_dev in glob(BLOCK_DEV_SYSFS_GLOB).unwrap() {
            let mut block_dev = phys_dev.unwrap().clone();
            block_dev.pop();
            let dev_base = block_dev.to_string_lossy();
            debug!(
                "link: {}",
                fs::canonicalize(format!("{}/device", dev_base))
                    .unwrap_or_default()
                    .display()
            );
            let block_dev = block_dev.file_name().unwrap_or_default().to_string_lossy();
            let serial = sysfs::string_attr(format!("{}/device/serial", dev_base));
            let model = sysfs::string_attr(format!("{}/device/model", dev_base));
            let uuid = sysfs::string_attr(format!("{}/uuid", dev_base));
            let wwid = sysfs::string_attr(format!("{}/wwid", dev_base));
            let capacity_bytes = sysfs::integer_attr(format!("{}/size", dev_base));
            let location = sysfs::path_by_device_link(&format!("{}/device", dev_base));
            let bus = sysfs::bus_by_device_link(&format!("{}/device", dev_base));
            // TODO: We also need to check for filesystems if there are no partitions
            let partitions = HoloPartitionInventory::from_host(&block_dev);
            let filesystem: Option<HoloFilesystemInventory> = if partitions.is_empty() {
                // No partitions, perhaps this block device contains a filesystem
                match parse_fs(&block_dev) {
                    Ok(fs) => Some(fs),
                    Err(_) => None,
                }
            } else {
                None
            };

            ret.push(HoloDriveInventory {
                block_dev: block_dev.to_string(),
                serial,
                model,
                wwid,
                uuid,
                bus,
                location,
                capacity_bytes,
                partitions,
                filesystem,
            })
        }
        ret
    }
}

/// A list of bus types for attaching devices to a host. Useful for finding USB stick block
/// devices, or identifying performance characteristics of a device. Note that a device could be
/// attached to multiple busses (PCI->USB->SCSI->storage), but this represents the
/// closest-attached, physical bus.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub enum InventoryBusType {
    /// PCI and PCI express
    PCI,
    /// USB
    USB,
    /// Serial ATA, we're unlikely to see Parallel ATA (PATA) anymore
    SATA,
    /// Serial SCSI
    SAS,
    /// System on Chip. Common on a lot of aarch64, arm32 and riscv platforms. Device is directly
    /// on the chip.
    SOC,
    /// Unknown bus type.
    #[default]
    UNKNOWN,
}

/// A representation of a partition on a drive, its attributes, and any recognised filesystems
/// contained within.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloPartitionInventory {
    /// Block device node for partition.
    pub block_dev: String,
    /// Partition number
    pub number: Option<u64>,
    /// Partition start block (512 byte blocks).
    pub start: Option<u64>,
    /// Partition length in 512-byte blocks.
    pub size: Option<u64>,
    /// Representation of the filesystem within this partition, if present and recognised.
    pub filesystem: Option<HoloFilesystemInventory>,
}

/// A collection of filesystem attributes from supported filesystems.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloFilesystemInventory {
    /// Filesystem label
    pub label: String,
    /// Filesystem UUID
    pub fsuuid: String,
    /// Filesystem last mount point.
    pub last_mount: String,
}

impl HoloPartitionInventory {
    pub fn from_host(phys_dev: &str) -> Vec<HoloPartitionInventory> {
        let mut ret: Vec<HoloPartitionInventory> = vec![];
        let glob_pattern = format!("/sys/class/block/{}/{}*/partition", phys_dev, phys_dev);

        debug!(
            "Looking for partitions of {} using glob {}",
            phys_dev, glob_pattern
        );

        if let Ok(parts) = glob(&glob_pattern) {
            for mut part in parts.flatten() {
                part.pop();
                let dev_base = part.to_string_lossy();
                // Using unwrap() or similar isn't ideal, but in this case should only fail if
                // we fail to decode the filename into a string. If this happens, we have
                // bigger problems than a failed inventory.
                let block_dev = part.file_name().unwrap_or_default().to_string_lossy();

                let fs = parse_fs(&block_dev);

                ret.push(HoloPartitionInventory {
                    block_dev: block_dev.to_string(),
                    number: sysfs::integer_attr(format!("{}/partition", dev_base)),
                    start: sysfs::integer_attr(format!("{}/start", dev_base)),
                    size: sysfs::integer_attr(format!("{}/size", dev_base)),
                    filesystem: fs.ok(),
                })
            }
        }

        ret
    }
}

/// A representation of a network interface card (NIC).
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloNicInventory {
    /// Network interface name in kernel.
    pub iface_dev: String,
    /// Hardware address, usually the MAC address.
    pub hwaddr: Option<String>,
    /// Hardware vendor ID. See `pci.ids` for mapping to a string.
    pub vendor: Option<String>,
    /// Hardware model ID. See `pci.ids` for mapping to a string.
    pub model: Option<String>,
    /// Bus that the device is attached to. PCI, for example.
    pub bus: InventoryBusType,
    /// Location within the hardware tree for the device.
    pub location: String,
}

impl HoloNicInventory {
    const NET_DEV_GLOB: &str = "/sys/class/net/*/device";
    pub fn from_host() -> Vec<HoloNicInventory> {
        let mut ret: Vec<HoloNicInventory> = vec![];

        for phys_dev in glob(Self::NET_DEV_GLOB).unwrap() {
            let mut net_dev = phys_dev.unwrap().clone();
            net_dev.pop();
            let dev_base = net_dev.to_string_lossy();
            debug!("Processing network device {}", dev_base);
            let iface_dev = net_dev
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let hwaddr = sysfs::string_attr(format!("{}/address", dev_base));
            let vendor = sysfs::string_attr(format!("{}/device/vendor", dev_base));
            let model = sysfs::string_attr(format!("{}/device/device", dev_base));
            let bus = sysfs::bus_by_device_link(&format!("{}/device", dev_base));
            let location = sysfs::path_by_device_link(&format!("{}/device", dev_base));

            ret.push(HoloNicInventory {
                iface_dev,
                hwaddr,
                vendor,
                model,
                bus,
                location,
            })
        }

        ret
    }
}

/// Data structure representing a node CPU. We currently only grab a few fields that we use
/// elsewhere, but will likely want to add to the list of CPU attributes we harvest.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloProcessorInventory {
    /// CPU vendor string
    pub vendor: String,
    /// CPU model string.
    pub model: String,
    /// CPU flags
    pub flags: Vec<String>,
}

impl HoloProcessorInventory {
    /// Path on Linux to /proc/cpuinfo. We have a few ways of getting access to the details of the
    /// attached CPUs. Methods like the CPUID instruction give a lot of great info (and this is
    /// where the Linux kernel gets info from for /proc/cpuinfo), but is x86_64 specific and even
    /// then varies between AMD and Intel brands. The cpuinfo file is pretty consistent across
    /// architectures and implementations.
    const LINUX_CPUINFO_PATH: &str = "/proc/cpuinfo";

    pub fn from_host() -> Vec<HoloProcessorInventory> {
        let mut ret: Vec<HoloProcessorInventory> = vec![];

        let file = File::open(Self::LINUX_CPUINFO_PATH).unwrap();
        let reader = io::BufReader::new(file);
        let cpuinfo = CpuInfo::from_buf_read(reader).unwrap();

        // XXX: This is currently very heavily tailored to x86_64 CPUs (Intel and AMD) and will
        // need some adjustment for aarch64 and riscv. This will likely just involve different map
        // key names in the cpuinfo map read above.
        //
        // This does already support cases where not all processors/CPUs/cores are the same within
        // a single machine (ARM's big.LITTLE for example).
        for core in 0..cpuinfo.num_cores() {
            // do stuff
            ret.push(HoloProcessorInventory {
                vendor: cpuinfo.vendor_id(core).unwrap_or_default().to_string(),
                model: cpuinfo.model_name(core).unwrap_or_default().to_string(),
                flags: cpuinfo
                    .flags(core)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|flag| flag.to_string())
                    .collect(),
            });
        }

        ret
    }
}

/// A struct representing a memory object
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Default, Clone)]
pub struct HoloMemoryInventory {
    pub size: u64,
    pub form_factor: String,
    pub memory_type: String,
    pub memory_speed: u16,
}

impl HoloMemoryInventory {
    const SMBIOS_MEMORY_PATH_GLOB: &str = "/sys/firmware/dmi/entries/17-*/raw";

    pub fn from_host() -> Vec<HoloMemoryInventory> {
        let mut ret: Vec<HoloMemoryInventory> = vec![];

        info!("Checking memory glob: {}", Self::SMBIOS_MEMORY_PATH_GLOB);

        match glob(Self::SMBIOS_MEMORY_PATH_GLOB) {
            Ok(mem_devs) => {
                for mem_dev in mem_devs {
                    debug!("Mem dev: {:?}", mem_dev);
                    match mem_dev {
                        Ok(mem) => {
                            let Ok(res) = sysfs::parse_mem_file(mem.to_string_lossy().as_ref())
                            else {
                                continue;
                            };
                            ret.push(res);
                        }
                        Err(e) => {
                            info!("Failed to parse mem dev: {}", e);
                        }
                    }
                }
            }
            Err(e) => info!("Memory glob failed with: {}", e),
        }

        ret
    }
}

/// This is the glob patch to match all OpenSSH host _public_ keys. We never touch the private key.
const SSHD_HOST_KEY_GLOB: &str = "/etc/ssh/ssh_host_*_key.pub";

/// Read/parse the SSH public keys and return.
fn ssh_host_keys() -> Vec<SSHPubKey> {
    let mut ret = vec![];

    for keypath in glob(SSHD_HOST_KEY_GLOB).unwrap() {
        let keyfile = keypath.unwrap().clone().to_string_lossy().to_string();
        debug!("Parsing SSH public key file {}", keyfile);
        match fs::read_to_string(&keyfile) {
            Ok(pubkey) => {
                let mut fields = pubkey.strip_suffix("\n").unwrap_or_default().split(" ");
                ret.push(SSHPubKey {
                    key_type: fields.next().unwrap_or_default().to_string(),
                    key: fields.next().unwrap_or_default().to_string(),
                    label: fields.next().unwrap_or_default().to_string(),
                })
            }
            Err(e) => {
                info!("Failed to read SSH host public key {}: {}", keyfile, e);
            }
        };
    }
    ret
}

/// Path to machine-id file, as specified in the FreeDesktop standards.
pub const MACHINE_ID_PATH: &str = "/etc/machine-id";

fn systemd_machine_id() -> String {
    match fs::read_to_string(MACHINE_ID_PATH).and_then(|content| {
        content
            .lines()
            .next()
            .ok_or_else(|| std::io::Error::new(io::ErrorKind::InvalidData, "no lines found"))
            .map(ToString::to_string)
    }) {
        Ok(id) => id,
        Err(e) => {
            info!("No systemd machine ID found at {}: {}", MACHINE_ID_PATH, e);
            "".to_string() // most inventory attributes are best-effort
        }
    }
}

/// Path to kernel version string
const KERNEL_VERSION_FILE: &str = "/proc/version";

fn linux_kernel_build() -> String {
    let ret = match fs::read_to_string(KERNEL_VERSION_FILE) {
        Ok(build) => build.strip_suffix("\n").unwrap_or_default().to_string(),
        Err(e) => {
            info!(
                "No kernel build file found at {}: {}",
                KERNEL_VERSION_FILE, e
            );
            return "".to_string();
        }
    };

    ret
}

const NIXOS_VERSION_WRAPPER: &str = "/run/current-system/sw/bin/nixos-version";
/// Get operating system distribution release string
fn get_distro_release() -> String {
    // See if we're running on NixOS and handle that case.
    if Path::new(NIXOS_VERSION_WRAPPER).exists() {
        // This is utterly heinous. Unlike any LSB-compliant Linux distribution, where we can
        // simply read a file to get the version string, NixOS makes us fork/exec symlinks to
        // wrapper scripts to shell scripts that have the release string hard-coded in them. The
        // inventory is supposed to be lightweight and we go to reasonable lengths to avoid this
        // collection just being a shell script written in Rust. Calling fork/exec like this is
        // only ever done as a last resort.
        let output = Command::new(NIXOS_VERSION_WRAPPER).output();
        match output {
            Ok(output) => match output.status.success() {
                true => {
                    return String::from_utf8_lossy(&output.stdout)
                        .strip_suffix("\n")
                        .unwrap_or_default()
                        .to_string()
                }
                false => return "Unknown NixOS release".to_string(),
            },
            Err(e) => {
                info!("Failed to execute nixos-version: {}", e);
                return "Unknown NixOS release".to_string();
            }
        }
    }

    // We'll add support for other distributions later for the sake of completeness. But this will
    // suffice for now.
    "Undetected".to_string()
}

const NIX_VERSION_WRAPPER: &str = "/run/current-system/sw/bin/nix";
/// Get primary package manager release string.
fn get_package_mgr_release() -> String {
    // See if we're running on NixOS and handle that case.
    if Path::new(NIX_VERSION_WRAPPER).exists() {
        // See above comment around the heinous nature of having to fork/exec wrappers to retrieve
        // a version string.
        let output = Command::new(NIX_VERSION_WRAPPER).arg("--version").output();
        match output {
            Ok(output) => match output.status.success() {
                true => {
                    return String::from_utf8_lossy(&output.stdout)
                        .strip_suffix("\n")
                        .unwrap_or_default()
                        .to_string()
                }
                false => return "Unknown Nix release".to_string(),
            },
            Err(e) => {
                info!("Failed to execute nix --version: {}", e);
                return "Unknown Nix release".to_string();
            }
        }
    }

    // We'll add support for other distributions later for the sake of completeness. But this will
    // suffice for now.
    "Undetected".to_string()
}
