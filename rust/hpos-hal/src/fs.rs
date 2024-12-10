use crate::inventory::{HoloFilesystemInventory, InventoryError, InventoryErrorInner};
use binrw::*;
use log::{debug, info};
use std::io::{prelude::*, SeekFrom};
use uuid::Uuid;

/// Subset of FAT32 metadata layout. Note that FAT16 and FAT12, used on smaller devices, are
/// different and won't match here.
#[derive(BinRead, Debug)]
#[br(little)]
struct Fat32Metadata {
    // BIOS parameter block. We don't need any of this information at all.
    _bios_parameter_block: [u8; 36],
    // The next fields vary depending on the flavour of FAT filesystem. Brlow is specific to FAT32.
    _sectors_per_fat: i32,
    _flags: i16,
    _fat_version: i16,
    _root_cluster: i32,
    _fsinfo_sector: i16,
    _backup_boot_sector: i16,
    _reserved_bytes: [u8; 12],
    _drive_number: i8,
    _nt_flags: i8,
    _signature: i8,
    serial_number: u32,
    volume_label: [u8; 11],
    // This is always supposed to be the string 'FAT32   ', but the spec says to not rely on it.
    _fat32_not_magic: [u8; 8],
    _boot_code: [u8; 420],
    #[br(magic = 0xaa55u16)]
    _magic: u32,
}

impl Fat32Metadata {
    pub fn label(&self) -> Result<String, InventoryError> {
        let label = std::str::from_utf8(&self.volume_label)?;
        let label = label.trim_matches(' ');
        Ok(label.to_string())
    }

    pub fn serial(&self) -> Result<String, InventoryError> {
        let serial = format!("{:X}", self.serial_number);
        Ok(serial.to_string())
    }
}

/// Subset of EXT4 Super Block structure
#[derive(BinRead, Debug)]
#[br(little)]
struct Ext4SuperBlock {
    _s_inodes_count: i32,
    _s_blocks_count_lo: i32,
    _s_r_blocks_count_lo: i32,
    _s_free_blocks_count_lo: i32,
    _s_free_inodes_count: i32,
    _s_first_data_block: i32,
    _s_log_block_size: i32,
    _s_log_cluster_size: i32,
    _s_blocks_per_group: i32,
    _s_clusters_per_group: i32,
    _s_inodes_per_group: i32,
    _s_mtime: i32,
    _s_wtime: i32,
    _s_mnt_count: i16,
    _s_max_mnt_count: i16,
    #[br(magic = 0xef53u16)]
    //_s_magic: i16,
    _s_state: i16,
    _s_errors: i16,
    _s_minor_rev_level: i16,
    _s_lastcheck: i32,
    _s_checkinterval: i32,
    _s_creator_os: i32,
    _s_rev_level: i32,
    _s_def_resuid: i16,
    _s_def_resgid: i16,
    _s_first_ino: i32,
    _s_inode_size: i16,
    _s_block_group_nr: i16,
    _s_feature_compat: i32,
    _s_feature_incompat: i32,
    _s_feature_ro_compat: i32,
    s_uuid: [u8; 16],
    s_volume_name: [u8; 16],
    s_last_mounted: [u8; 64],
    _s_algorithm_usage_bitmap: i32,
}

impl Ext4SuperBlock {
    pub fn label(&self) -> Result<String, InventoryError> {
        let label = std::str::from_utf8(&self.s_volume_name)?;
        let label = label.trim_matches('\0');
        Ok(label.to_string())
    }

    pub fn fsuuid(&self) -> Result<String, InventoryError> {
        // TODO: This is wrong. It's encoded some other way.
        let fsuuid = Uuid::from_bytes(self.s_uuid);
        Ok(fsuuid.to_string())
    }

    pub fn last_mount(&self) -> Result<String, InventoryError> {
        let mount = std::str::from_utf8(&self.s_last_mounted)?;
        let mount = mount.trim_matches('\0');
        Ok(mount.to_string())
    }
}

pub fn parse_fs(block_dev: &str) -> Result<HoloFilesystemInventory, InventoryError> {
    // TODO: This should use a read with a timeout in case the block device is inaccessible. We
    // don't want to block forever.
    debug!("Looking at {} for known filesystems", block_dev);
    let path = format!("/dev/{}", block_dev);
    let ext4 = try_ext4(&path);
    match ext4 {
        Ok(ext4) => return Ok(ext4),
        Err(e) => {
            info!("Failed to find EXT4 on {}: {}", block_dev, e.to_string());
        }
    }

    let fat32 = try_fat32(&path);
    match fat32 {
        Ok(fat32) => return Ok(fat32),
        Err(e) => {
            info!("Failed to find FAT32 on {}: {}", block_dev, e.to_string());
        }
    }

    Err(InventoryError::Base(InventoryErrorInner::NotFound))
}

/// The EXT4 Super block always starts 1024 bytes into the block device.
const EXT4_SUPERBLOCK_LOCATION: u64 = 1024;

pub fn try_ext4(block_dev: &str) -> Result<HoloFilesystemInventory, InventoryError> {
    debug!("Checking {} for EXT4 filesystem", block_dev);
    let mut f = std::fs::File::open(block_dev)?;
    debug!("Seeking {} bytes into the device", EXT4_SUPERBLOCK_LOCATION);
    f.seek(SeekFrom::Start(EXT4_SUPERBLOCK_LOCATION))?;
    debug!("Parsing Superblock");
    let ext4_ret = f.read_ne::<Ext4SuperBlock>();
    let ext4 = match ext4_ret {
        Ok(ext4) => {
            //dbg!(&ext4);
            info!(
                "EXT4 label on {} is '{}', FSUUID '{}', last mounted at '{}'",
                block_dev,
                ext4.label()?,
                ext4.fsuuid()?,
                ext4.last_mount()?
            );
            ext4
        }
        Err(e) => {
            info!("Failed to parse EXT4 superblock: {}", e);
            return Err(e.into());
        }
    };
    Ok(HoloFilesystemInventory {
        label: ext4.label()?,
        fsuuid: ext4.fsuuid()?,
        last_mount: ext4.last_mount()?,
    })
}

const FAT32_METADATA_LOCATION: u64 = 0;

pub fn try_fat32(block_dev: &str) -> Result<HoloFilesystemInventory, InventoryError> {
    let mut f = std::fs::File::open(block_dev)?;
    f.seek(SeekFrom::Start(FAT32_METADATA_LOCATION))?;
    let fat32_ret = f.read_ne::<Fat32Metadata>();
    let fat32 = match fat32_ret {
        Ok(fat32) => {
            info!("FAT32 label on {} is '{}'", block_dev, fat32.label()?);
            fat32
        }
        Err(e) => {
            info!("Failed to parse FAT32 metadata block: {}", e);
            return Err(e.into());
        }
    };

    Ok(HoloFilesystemInventory {
        label: fat32.label()?,
        fsuuid: fat32.serial()?,
        last_mount: "".to_string(), // Doesn't apply to FAT32
    })
}
