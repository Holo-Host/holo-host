//! This crate provides soem data types that implement a local content addressable _local_ blob
//! store. This implements the low-level persistence layer for a blob store that will be present on
//! many machines throughout the network. It provides opaque access to a local repository of
//! content addressable blobs, through interfaces that implement the [std::io::Read] and
//! [std::io::Write] Traits.
//!
//! The primary interface is the [LocalContentAddressableBlobStore] type, which has examples
//! included.
//!
//! It is anticipated that there will be other layers built on top of this that will handle data
//! distribution and sharding, redundancy through erasure coding and similar, and more.
use base64::prelude::*;
use bitmask_enum::bitmask;
use blake3::Hasher;
use log::debug;
use serde_derive::{Deserialize, Serialize};
use std::fmt;
use std::fs::{create_dir_all, rename, File, OpenOptions};
use std::io::{Error, Read, Write};
use std::path::Path;
use std::time::SystemTime;

mod tests;

/// Blob-Store-wide features. Includes Per-Blob defaults to be set on new blobs when added to the
/// blob store, including things like transparent compression and encryption. Not currently
/// implemented, but added as a placeholder to avoid having to modify the data structures later.
#[bitmask]
#[derive(Serialize, Deserialize)]
enum BlobStoreFeatures {
    /// Enable transparent/inline compression for new blobs added.
    InlineCompression,
    /// Enable transparent/inline encryption for new blobs added.
    InlineEncryption,
}

/// An internal representation of metadata for a given blob store. This will likely later include
/// mention of specific features (EC, transparent encryption, transparent compression, etc).
#[derive(Debug, Serialize, Deserialize)]
struct LocalBlobStoreMetadata {
    version: usize,
    created: SystemTime,
    features: BlobStoreFeatures,
}

/// Current blob store format version. For the future.
const BLOB_STORE_VERSION: usize = 1;

impl LocalBlobStoreMetadata {
    /// Generate a new blob store metadata object
    fn new() -> LocalBlobStoreMetadata {
        LocalBlobStoreMetadata {
            version: BLOB_STORE_VERSION,
            created: SystemTime::now(),
            features: BlobStoreFeatures::none(),
        }
    }

    /// Persist a blob store metadata object
    fn write(&self, path: &str) -> Result<(), Error> {
        let json_data = serde_json::to_string(&self)?;
        let mut md_file = File::create(path)?;
        md_file.write_all(json_data.as_bytes())
    }

    /// Read, parse and return an existing blob store metadata object.
    fn from_file(path: &str) -> Result<LocalBlobStoreMetadata, Error> {
        let mut md_file = File::open(path)?;
        let mut data = String::new();
        md_file.read_to_string(&mut data)?;
        Ok(serde_json::from_str(&data)?)
    }
}

/// LocalContentAddressableBlobStore represents a managed repository for storing of content-addressable
/// blobs. It ought to be possible for a single process to instantiate multiple blob stores,
/// paving the way for the ability to have different storage backends on one node (for tiering), or
/// to be able to seamlessly migrate data from an older-version format to a newer format.
///
/// Here's an example writing a blob to a blob store repo:
///
/// ```rust
/// use std::io::Write;
/// use holo_blobstore::LocalContentAddressableBlobStore;
/// // Owner is opaque to us, but the API tracks an owner(s) of a blob.
/// let owner = "590c6197-0b1e-4469-b2b7-6a9144b33f47";
/// // Get a handle to the blob store.
/// let store = LocalContentAddressableBlobStore::init("/tmp").unwrap();
/// // Get a handle to a new entry in the blob store.
/// let mut blob = store.create_blob_writer(owner.to_string()).unwrap();
/// // Read/generate our source data.
/// let mut data: Vec<String> = vec![];
/// data.push("This".to_string());
/// data.push("Is".to_string());
/// data.push("Content".to_string());
/// // Write the data.
/// for line in data.iter() {
///     blob.write(line.as_bytes()).unwrap();
/// }
/// // All-important commit to finish calculating the content ID.
/// let cid = store.finalise(&mut blob).unwrap();
/// println!("CID: {}", cid.to_string());
/// ```
///
/// Here's an example reading from a blob store:
///
/// ```rust
/// use std::io::Read;
/// use holo_blobstore::LocalContentAddressableBlobStore;
/// // A string representation of the Content ID
/// let cid = "c1okNokAekfyyLSiZXrB07i2SwbqQhs1Te-WqAKhKjsGM";
/// // Open a handle to the local blob store
/// let store = LocalContentAddressableBlobStore::init("/tmp").unwrap();
/// // Retrieve a [Read] Trait capable handle for the blob.
/// let mut blob = store.get_blob_reader(cid).unwrap();
/// // Read the data into a string.
/// let mut data = String::new();
/// blob.read_to_string(&mut data).unwrap();
/// // Once we've read the whole file, we can check its hash against our CID
/// blob.verify(&cid).unwrap();
/// println!("Data verified: {}", data);
/// ```
#[derive(Debug)]
pub struct LocalContentAddressableBlobStore {
    path: String,
    metadata: LocalBlobStoreMetadata,
}

impl LocalContentAddressableBlobStore {
    const REPO_DATA_DIR: &str = "v1_data";
    const REPO_TMP_DIR: &str = "new";

    /// Initialises a handle to a blob store instance, creating it if it doesn't exist.
    pub fn init(path: &str) -> Result<LocalContentAddressableBlobStore, Error> {
        // check that the store metadata makes sense, or create it if missing
        let store_root = Path::new(path);
        if !store_root.is_dir() {
            // Create all path components to the blob store root. We also want a directory called
            // `new`, where we'll create temporary files for new blobs before renaming them to have
            // their content ID as their filename.
            //
            // The V1 repo is super naive. It has a directory for temporarily housing new files,
            // and a directory for objects once they're complete and committed, where they'll take
            // their CID as their local filesystem filename. This is naive because most filesystems
            // start to perform poorly with tens or hundreds of thousands of objects in a single
            // directory. Much better would be to shard the files into a couple of levels of
            // subdirectory by the first few characters of their CID hash. The current approach
            // will suffice for now, and creating a background migration path to a sharded V2 repo
            // format ought to be fairly trivial.
            debug!("Initialising new blob store repository under {}", path);
            create_dir_all(path)?;
            //create_dir_all(format!("{}/{}", path, Self::REPO_DATA_DIR))?;
            //create_dir_all(format!("{}/{}", path, Self::REPO_TMP_DIR))?;
            let metadata = LocalBlobStoreMetadata::new();
            metadata.write(&format!("{}/metadata.json", path))?;
        }

        for subdir in [Self::REPO_DATA_DIR, Self::REPO_TMP_DIR] {
            let sub_path = store_root.join(subdir);
            if !sub_path.is_dir() {
                create_dir_all(sub_path)?;
            }
        }

        let metadata_filename = format!("{}/metadata.json", path);
        let metadata_file = Path::new(&metadata_filename);
        if !metadata_file.is_file() {
            debug!(
                "Initialising missing blob store repo metadata at {}",
                metadata_filename
            );
            let metadata = LocalBlobStoreMetadata::new();
            metadata.write(&metadata_file.to_string_lossy())?;
        }

        // At this point, we should have a metadata block regardless of whether it's a new or
        // existing repo.
        debug!("Opened blob store repo at {}", path);
        Ok(LocalContentAddressableBlobStore {
            path: path.to_string(),
            metadata: LocalBlobStoreMetadata::from_file(&metadata_filename)?,
        })
    }

    /// Returns a [Write] trait enabled handle to a new entry in the blob store. The owner is
    /// opaque to us. The API needs to record owner(s) of blobs, so we persist them along with the
    /// file's metadata.
    pub fn create_blob_writer(&self, owner: String) -> Result<ContentAddressableBlob, Error> {
        debug!(
            "Creating object for {} under blob store {}, version {}.",
            &owner, self.path, self.metadata.version
        );
        // We won't know the end filename until we have seen all of the data. Initially, we create
        // a file with a unique name in the temporary directory part of the repo. When we do a
        // final commit to the file, we rename the file to a different part of the repo and using
        // its content ID/hash as its filename.
        //
        // TODO: When we open the file, we should probably delete it immediately. This will mean
        // that if someone opens a file and writes a bunch of data to it and never commits the
        // data, we don't end up with orphaned objects lying around in the temporary space, which
        // could be used as a denial-of-service attack at its extreme.
        //
        // However, there isn't a way to rename a deleted file, as rename system and library calls
        // are generally done using filenames and the directory entry for the file doesn't actually
        // exist when we try to do the rename. It's worth us taking a look to see whether there's a
        // way to hard-link or rename, or otherwise write a directory entry, for a file using just
        // its open file handle and not a source directory path. Something like the Linux-specific
        // `linkat(3)` system call may suffice here, but given that it takes a string to the old
        // path, I suspect it's not sufficient.
        let mut g = srfng::Generator::new();
        let tmpfile = format!(
            "{}/{}/{}",
            self.path,
            Self::REPO_TMP_DIR,
            g.generate().as_str()
        );
        debug!("Creating blob at {}", tmpfile);
        ContentAddressableBlob::new(&tmpfile, &owner)
    }

    /// Returns a [Read] trait enabled handle to a new entry in the blob store.
    pub fn get_blob_reader(&self, cid: &str) -> Result<ContentAddressableBlob, Error> {
        debug!(
            "Reading object with CID {} under blob store {}, version {}.",
            &cid, self.path, self.metadata.version
        );
        // First parse the CID (using the [ContentID] type) and then check to see
        // if the file exists in the repo. If it does exist, it should open the file read-only via
        // the [ContentAddressableBlob] type, returning the [ContentAddressableBlob].
        let blob_path = format!("{}/{}/{}", self.path, Self::REPO_DATA_DIR, cid);
        ContentAddressableBlob::get(&blob_path)
    }

    pub fn finalise(&self, blob: &mut ContentAddressableBlob) -> Result<ContentID, Error> {
        let cid = blob.commit()?;
        if let Some(src) = &blob.tmpfile {
            // Rename the file into the core of the repo and using its content ID as its filename.
            // We use rename because it's atomic.
            let dest = format!("{}/{}/{}", self.path, Self::REPO_DATA_DIR, cid);
            debug!("Finalising {} from {}", dest, src);
            rename(src, &dest)?;

            // Read existing metadata, if it exists, otherwise create a new set of metadata and
            // save it. We don't currently really use the metadata for anything yet, but this gets
            // us a headstart on newly-created objects.
            let metadata_filename = format!("{}.json", &dest);
            let metadata_path = Path::new(&metadata_filename);
            let mut metadata: BlobMetadata = match metadata_path.is_file() {
                true => BlobMetadata::from_file(&metadata_filename)?,
                false => BlobMetadata::new(),
            };

            // Update fields
            metadata.updated = SystemTime::now();
            metadata.owners.push(blob.owner.clone());

            // Write new/updated metadata file.
            metadata.write(&metadata_filename)?;
        } else {
            // We shouldn't be able to get to this point.
            debug!("Temporary source file for {} doesn't exist?", cid);
            return Err(std::io::ErrorKind::NotFound.into());
        }

        Ok(cid)
    }
}

/// ContentAddressableBlob represents a blob being written to, or read from, a
/// [LocalContentAddressableBlobStore]. It transparently handles the creation of the content ID from the
/// content, and validating the content when read back.
///
/// This is likely not an interface you'll want to deal with directly. Instead, use the
/// [LocalContentAddressableBlobStore] interface and let it return [ContentAddressableBlob] handles to
/// perform I/O on.
#[derive(Debug)]
pub struct ContentAddressableBlob {
    handle: File,
    hasher: Hasher,
    tmpfile: Option<String>,
    owner: String,
}

impl ContentAddressableBlob {
    /// Returns a write-only handle to a new entry in the blob store. The handle supports the
    /// [Write] trait and handles transparent hashing of the content to create a [ContentID] once
    /// the file has been written.
    fn new(tmpfile: &str, owner: &str) -> std::result::Result<ContentAddressableBlob, Error> {
        debug!("Creating {}", tmpfile);
        let ret = ContentAddressableBlob {
            handle: OpenOptions::new()
                .read(false)
                .write(true)
                .create(true)
                .truncate(true)
                .open(tmpfile)?,
            hasher: Hasher::new(),
            tmpfile: Some(tmpfile.to_string()),
            owner: owner.to_string(),
        };
        debug!("Created blob: {:?}", &ret);
        Ok(ret)
    }

    /// Finalises the blob's hasher and returns the [ContentID] of the blob.
    pub(crate) fn commit(&mut self) -> std::result::Result<ContentID, Error> {
        self.flush()?;
        let hash = self.hasher.finalize();
        // base64 gives us a smaller string than a hex representation of the string. Base91 might
        // be better, but has additional characters that might need to be escaped in some
        // languages, tools or interfaces.
        let b64 = BASE64_URL_SAFE_NO_PAD.encode(hash.as_bytes());
        Ok(ContentID::new_v1_from_hash(&b64))
    }

    /// Returns a handle to the blob that has the [Read] trait.
    fn get(path: &str) -> Result<ContentAddressableBlob, Error> {
        debug!("Reading blob from {}", path);
        Ok(ContentAddressableBlob {
            handle: OpenOptions::new().read(true).write(false).open(path)?,
            hasher: Hasher::new(),
            tmpfile: None,
            owner: String::new(),
        })
    }

    pub fn verify(&self, cid: &str) -> std::result::Result<ContentID, Error> {
        let hash = self.hasher.finalize();
        let b64 = BASE64_URL_SAFE_NO_PAD.encode(hash.as_bytes());
        let blob_cid = ContentID::new_v1_from_hash(&b64);
        debug!("Validating CID matches content: {} == {}", cid, blob_cid);
        match cid == blob_cid.to_string() {
            true => Ok(blob_cid),
            false => Err(std::io::ErrorKind::InvalidData.into()),
        }
    }
}

impl Read for ContentAddressableBlob {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let ret = self.handle.read(buf);
        match ret {
            Ok(size) => {
                if size > 0 {
                    // only update if the buffer is updated, and don't assume that the buffer was
                    // totally filled.
                    self.hasher.update(&buf[0..size]);
                }
                Ok(size)
            }
            Err(e) => Err(e),
        }
    }
}

impl Write for ContentAddressableBlob {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.hasher.update(buf);
        self.handle.write(buf)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.handle.flush()
    }
}

/// A bitmask enum representing possible features for a given stored blob. Currently not
/// implemented, but added now to avoid changes to the on-disk data structures later.
#[bitmask]
#[derive(Serialize, Deserialize)]
enum BlobStorageFeatures {
    InlineCompression,
    InlineEncryption,
}

/// Current blob store format version. For the future.
#[derive(Debug, Serialize, Deserialize)]
enum BlobMetadataVersion {
    VERSION1,
}

/// An internal representation of metadata for a given data blob. Multiple parties could upload the
/// same blob, in which case, we'll have a single blob on disk, with multiple owners. Hence the
/// [owners] field being a vector.
///
/// In the future, this could also support per-blob features, such as transparent encryption,
/// transparent compression, etc.
#[derive(Debug, Serialize, Deserialize)]
struct BlobMetadata {
    version: BlobMetadataVersion,
    created: SystemTime,
    updated: SystemTime,
    owners: Vec<String>,
    features: BlobStorageFeatures,
}

impl BlobMetadata {
    /// Generate a new blob store metadata object
    fn new() -> BlobMetadata {
        BlobMetadata {
            version: BlobMetadataVersion::VERSION1,
            created: SystemTime::now(),
            updated: SystemTime::now(),
            owners: vec![],
            features: BlobStorageFeatures::none(),
        }
    }

    /// Persist a blob store metadata object
    fn write(&self, path: &str) -> Result<(), Error> {
        let json_data = serde_json::to_string(&self)?;
        let mut md_file = File::create(path)?;
        md_file.write_all(json_data.as_bytes())
    }

    /// Read, parse and return an existing blob store metadata object.
    fn from_file(path: &str) -> Result<BlobMetadata, Error> {
        let mut md_file = File::open(path)?;
        let mut data = String::new();
        md_file.read_to_string(&mut data)?;
        Ok(serde_json::from_str(&data)?)
    }
}

/// ContentID is a simple versioned wrapper around a content ID to make it easier to version and
/// parse.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentID {
    V1 { cid: String },
}

impl ContentID {
    fn new_v1_from_hash(id: &str) -> ContentID {
        ContentID::V1 {
            cid: format!("c1{}", id),
        }
    }
}

impl fmt::Display for ContentID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::V1 { cid } => write!(f, "{}", cid),
        }
    }
}
