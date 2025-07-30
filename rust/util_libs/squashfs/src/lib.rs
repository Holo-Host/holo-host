use backhand::{FilesystemReader, FilesystemWriter, InnerNode, NodeHeader};
use std::{fs, io::Read, io::Write, path::Path};

fn list_files_recursively(path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.push(format!("{}/", path.to_string_lossy()));
                files.extend(list_files_recursively(&path));
            } else {
                files.push(path.to_string_lossy().to_string());
            }
        }
    }
    files
}

/// create squashfs archive using files
pub fn create_archive(source_dir: String, output_path: String) -> Result<(), std::io::Error> {
    let files = list_files_recursively(Path::new(&source_dir));

    let header = NodeHeader::default();
    let mut writer = FilesystemWriter::default();
    for file in files {
        let reader = fs::File::open(file.clone())?;
        let file_path = file.replace(&source_dir, "");
        if file_path.ends_with("/") {
            writer.push_dir(file_path, header)?;
        } else {
            writer.push_file(reader, file_path, header)?;
        }
    }
    let mut output = fs::File::create(output_path)?;
    writer.write(&mut output)?;
    Ok(())
}

/// unarchive squashfs into a folder
pub fn unpack_archive(package_path: String, output_path: String) -> Result<(), std::io::Error> {
    let package_files = list_files(package_path.clone())?;
    for package_file in package_files {
        if package_file.ends_with("/") {
            fs::create_dir_all(Path::new(&format!("{}{}", output_path, package_file)))?;
            continue;
        }
        let file_contents = read_file(package_path.clone(), package_file.clone())?;
        let file_path = format!("{}{}", output_path, package_file);
        let mut file = fs::File::create(file_path)?;
        if file_contents.is_some() {
            let file_contents = file_contents.unwrap();
            file.write_all(&file_contents)?;
        }
    }
    Ok(())
}

pub fn list_files(squashfs_path: String) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    let reader = fs::File::open(squashfs_path)?;
    let buf_reader = std::io::BufReader::new(reader);
    let reader = FilesystemReader::from_reader(buf_reader)?;
    for node in reader.files() {
        if let InnerNode::File(_) = node.inner {
            files.push(node.fullpath.to_string_lossy().to_string())
        } else {
            files.push(format!("{}/", node.fullpath.to_string_lossy()))
        }
    }
    Ok(files)
}

pub fn read_file(
    squashfs_path: String,
    file_path: String,
) -> Result<Option<Vec<u8>>, std::io::Error> {
    let mut file = fs::File::open(squashfs_path)?;
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
