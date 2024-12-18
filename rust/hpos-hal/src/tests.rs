#[cfg(test)]
mod tests {
    use crate::inventory::HoloInventory;
    use std::process::Command;

    #[test]
    fn from_host() {
        let _inv = HoloInventory::from_host();
        //eprintln!("Inventory: {:?}", inv);
    }

    #[test]
    fn parse_fat32() {
        std::fs::create_dir_all("target").unwrap();

        let path = "target/vfat.img";
        let _out = Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", path))
            .arg("bs=1024k")
            .arg("count=2")
            .output()
            .unwrap();
        let _out = Command::new("mkfs.vfat")
            .arg("-n")
            .arg("POKEY")
            .arg("-i")
            .arg("DEADBEEF")
            .arg("-F")
            .arg("32")
            .arg(path)
            .output()
            .unwrap();
        let ret = crate::fs::try_fat32(path).unwrap();

        assert_eq!(ret.label, "POKEY");
        assert_eq!(ret.fsuuid, "DEADBEEF");
    }

    #[test]
    fn parse_ext4() {
        std::fs::create_dir_all("target").unwrap();

        let path = "target/ext4.img";
        // Create a sparse file so that we don't end up using many actual blocks, even for a
        // larger-sized filesystem. The `truncate` utility makes this easier, but we pull in `dd`
        // as a dependency above, so may as well use that.
        let _out = Command::new("dd")
            .arg("if=/dev/zero")
            .arg(format!("of={}", path))
            .arg("bs=1")
            .arg("count=1")
            .arg("seek=512M")
            .output()
            .unwrap();
        // Enough space for EXT4 to breathe, but without taking up any real space.
        // matthew@prickle:~/src/holo/hpos-hal$ du -hs target/ext4.img
        // 4.0K	target/ext4.img
        // matthew@prickle:~/src/holo/hpos-hal$ du -hs --apparent-size target/ext4.img
        // 513M	target/ext4.img
        let _out = Command::new("mkfs.ext4")
            .arg("-t")
            .arg("ext4")
            .arg("-L")
            .arg("Gumby")
            .arg("-U")
            .arg("195b9bfe-126b-49cf-970d-9184c0fe090c")
            .arg(&path)
            .output()
            .unwrap();

        let ret = crate::fs::try_ext4(path).unwrap();

        assert_eq!(ret.label, "Gumby");
        assert_eq!(ret.fsuuid, "195b9bfe-126b-49cf-970d-9184c0fe090c")
    }

    #[test]
    fn machine_id() {
        let inv = HoloInventory::from_host();

        let mut machine_cmd = assert_cmd::Command::new("systemd-machine-id-setup");
        let assert = machine_cmd.arg("--print").assert();
        assert.stdout(format!("{}\n", inv.system.machine_id));
    }

    #[cfg(feature = "tests_sudo")]
    #[test]
    fn smbios_bios() {
        let inv = HoloInventory::from_host();

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("bios-release-date")
            .assert();
        let bios_date = match inv.smbios.bios_date {
            Some(bios_date) => bios_date,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", bios_date));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("bios-revision")
            .assert();
        let bios_release = match inv.smbios.bios_release {
            Some(bios_release) => bios_release,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", bios_release));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("bios-version")
            .assert();
        let bios_version = match inv.smbios.bios_version {
            Some(bios_version) => bios_version,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", bios_version));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("bios-vendor")
            .assert();
        let bios_vendor = match inv.smbios.bios_vendor {
            Some(bios_vendor) => bios_vendor,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", bios_vendor));
    }

    #[cfg(feature = "tests_sudo")]
    #[test]
    fn smbios_board() {
        let inv = HoloInventory::from_host();

        /*
        * XXX: dmidecode replaces the empty string with 'Not Specified'. Fix this later.
        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("baseboard-asset-tag")
            .assert();
        let board_asset_tag = match inv.smbios.board_asset_tag {
            Some(board_asset_tag) => board_asset_tag,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", board_asset_tag));*/

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("baseboard-manufacturer")
            .assert();
        let board_vendor = match inv.smbios.board_vendor {
            Some(board_vendor) => board_vendor,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", board_vendor));

        // XXX: This test fails, claiming that dmidecode has no output on stdout, but running the
        // same commmand from an interactive shell works. :-|
        /*let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("baseboard-serial-number")
            .assert();
        let board_serial = match inv.smbios.board_serial {
            Some(board_serial) => board_serial,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", board_serial));*/

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("baseboard-version")
            .assert();
        let board_version = match inv.smbios.board_version {
            Some(board_version) => board_version,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", board_version));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("baseboard-product-name")
            .assert();
        let board_name = match inv.smbios.board_name {
            Some(board_name) => board_name,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", board_name));
    }

    // dmidecode translates raw values into strings for a lot of these parameters. These tests will
    // need to be altered for them to work.
    #[test]
    #[ignore]
    fn smbios_chassis() {
        let inv = HoloInventory::from_host();

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("chassis-serial-number")
            .assert();
        let chassis_serial = match inv.smbios.chassis_serial {
            Some(chassis_serial) => chassis_serial,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", chassis_serial));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("chassis-type")
            .assert();
        let chassis_type = match inv.smbios.chassis_type {
            Some(chassis_type) => chassis_type,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", chassis_type));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("chassis-manufacturer")
            .assert();
        let chassis_vendor = match inv.smbios.chassis_vendor {
            Some(chassis_vendor) => chassis_vendor,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", chassis_vendor));

        let mut dmi_cmd = assert_cmd::Command::new("sudo");
        let assert = dmi_cmd
            .arg("dmidecode")
            .arg("-s")
            .arg("chassis-version")
            .assert();
        let chassis_version = match inv.smbios.chassis_version {
            Some(chassis_version) => chassis_version,
            None => "".to_string(),
        };
        assert.stdout(format!("{}\n", chassis_version));
    }
}
