use backhand::{FilesystemReader, FilesystemWriter, InnerNode, NodeHeader};
use std::{fs::File, io::Read};

/// create squashfs archive using files
pub fn create_archive(
    output_path: String,
    files: Vec<String>,
    root_dir: String,
) -> Result<(), std::io::Error> {
    let header = NodeHeader::default();
    let mut writer = FilesystemWriter::default();
    for file in files {
        let reader = File::open(file.clone())?;
        let file_path = file.replace(&root_dir, "");
        writer.push_file(reader, file_path, header)?;
    }
    let mut output = File::create(output_path)?;
    writer.write(&mut output)?;
    Ok(())
}

pub fn list_files(squashfs_path: String) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    let reader = File::open(squashfs_path)?;
    let buf_reader = std::io::BufReader::new(reader);
    let reader = FilesystemReader::from_reader(buf_reader)?;
    for node in reader.files() {
        files.push(node.fullpath.to_string_lossy().to_string());
    }
    Ok(files)
}

pub fn read_file(
    squashfs_path: String,
    file_path: String,
) -> Result<Option<Vec<u8>>, std::io::Error> {
    // TODO: check if file exists
    let mut file = File::open(squashfs_path)?;
    let buf_reader = std::io::BufReader::new(&mut file);
    let reader = FilesystemReader::from_reader(buf_reader)?;

    let mut file_reader: Option<backhand::SquashfsReadFile> = None;
    for node in reader.files() {
        if let InnerNode::File(file) = &node.inner {
            let reader = reader.file(file).reader();
            if node.fullpath.to_string_lossy() == file_path {
                file_reader = Some(reader);
                break;
            }
        }
    }
    Ok(match file_reader {
        None => None,
        Some(mut reader) => {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;
            Some(buf)
        }
    })
}

#[cfg(test)]
mod tests;
