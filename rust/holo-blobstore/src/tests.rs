#[cfg(test)]
mod cidtests {
    use crate::ContentID;

    /// Trivial test of the first iteration of the CID type.
    #[test]
    fn v1_cid() {
        let cid = ContentID::new_v1_from_hash("xxxyyy");
        assert_eq!(cid.to_string(), "c1xxxyyy");
    }
}

#[cfg(test)]
mod blobstoretests {
    use crate::LocalContentAddressableBlobStore;
    use log::debug;
    use std::io::Read;
    use std::io::Write;
    use std::path::Path;
    use std::sync::Once;
    use test_temp_dir::test_temp_dir;

    static INIT: Once = Once::new();

    // Ensure that the logging is only initialised once.
    fn setup() {
        INIT.call_once(|| {
            env_logger::init();
        })
    }

    /// This test ensures that we can write a blob and then write the same data a second time.
    /// Given that the blob store will write the data and then rename the file to a filename that
    /// matches its content ID, there could be a case (bug) where the attempt to rename fails
    /// because the destination filename already exists.
    #[test]
    fn local_store_write_write() {
        setup();
        let owner = "590c6197-0b1e-4469-b2b7-6a9144b33f47".to_string();
        let tempdir = test_temp_dir!();
        let store =
            LocalContentAddressableBlobStore::init(tempdir.as_path_untracked().to_str().unwrap())
                .unwrap();
        let mut blob = store.create_blob_writer(owner.clone()).unwrap();
        let mut data: Vec<String> = vec![];
        data.push("This".to_string());
        data.push("Is".to_string());
        data.push("Content".to_string());
        for line in data.iter() {
            blob.write(line.as_bytes()).unwrap();
        }
        // We call the repo's finalise method, to commit the blob to the repo.
        let cid1 = store.finalise(&mut blob).unwrap();
        debug!("CID1: {}", cid1.to_string());

        // Taunt it a second time.
        let store =
            LocalContentAddressableBlobStore::init(tempdir.as_path_untracked().to_str().unwrap())
                .unwrap();
        let mut blob = store.create_blob_writer(owner.clone()).unwrap();
        // use identical data to above
        for line in data.iter() {
            blob.write(line.as_bytes()).unwrap();
        }
        // This shouldn't panic on the blob store rename().
        let cid2 = store.finalise(&mut blob).unwrap();
        debug!("CID2: {}", cid2.to_string());
        assert_eq!(cid1, cid2);
    }

    /// This test does a simple write, and then confirms that the blob exists in the repo with the
    /// CID as its filename
    #[test]
    fn local_store_write_check_path() {
        setup();
        let owner = "590c6197-0b1e-4469-b2b7-6a9144b33f47".to_string();
        let tempdir = test_temp_dir!();
        let store =
            LocalContentAddressableBlobStore::init(tempdir.as_path_untracked().to_str().unwrap())
                .unwrap();
        let mut blob = store.create_blob_writer(owner.clone()).unwrap();
        let mut data: Vec<String> = vec![];
        data.push("This".to_string());
        data.push("Is".to_string());
        data.push("Content".to_string());
        for line in data.iter() {
            blob.write(line.as_bytes()).unwrap();
        }
        // We call the repo's finalise method, to commit the blob to the repo.
        let cid = store.finalise(&mut blob).unwrap();
        debug!("CID: {}", cid.to_string());

        let assumed_filename = &format!(
            "{}/{}/{}",
            tempdir.as_path_untracked().to_str().unwrap(),
            LocalContentAddressableBlobStore::REPO_DATA_DIR,
            cid.to_string()
        );

        let assumed_path = Path::new(&assumed_filename);

        assert!(assumed_path.is_file());
    }

    /// This test writes some data and ensures that we can use the read interface to retrieve the
    /// object by its content ID, and validating the data read.
    #[test]
    fn local_store_write_read() {
        setup();
        let owner = "590c6197-0b1e-4469-b2b7-6a9144b33f47".to_string();
        let tempdir = test_temp_dir!();
        let store =
            LocalContentAddressableBlobStore::init(tempdir.as_path_untracked().to_str().unwrap())
                .unwrap();
        let mut blob = store.create_blob_writer(owner).unwrap();
        let mut data: Vec<String> = vec![];
        data.push("This".to_string());
        data.push("Is".to_string());
        data.push("Different".to_string());
        data.push("Content".to_string());
        for line in data.iter() {
            blob.write(line.as_bytes()).unwrap();
        }
        // We call the repo's finalise method, to commit the blob to the repo.
        let cid = store.finalise(&mut blob).unwrap();
        debug!("CID: {}", cid.to_string());

        // Retrieve a [Read] Trait capable handle for the blob.
        let mut blob = store.get_blob_reader(&cid.to_string()).unwrap();
        // Read the data into a string.
        let mut data = Vec::new();
        blob.read_to_end(&mut data).unwrap();
        // Once we've read the whole file, we can check its hash against our CID
        blob.verify(&cid.to_string()).unwrap();
    }
}
