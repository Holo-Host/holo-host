# hpos-hal

## Test Dependencies

The following binaries should be present to facilitate testing:

* `dmidecode` -- used to retrieve SMBIOS data from the DMI pool on most x86* machines
* `sudo` -- used to run tools such as dmidecode as root (needed to read `/dev/mem`).
* `mkfs.vfat` -- used to create a VFAT filesystem to test against.
* `mkfs.ext4` -- used to create an EXT4 filesystem to test against.
* `sh` and `dd` -- used along with mkfs.* to create small test filesystems.
