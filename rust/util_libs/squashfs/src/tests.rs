use super::*;
use std::fs;
use std::io::Write;
use std::path::Path;

// Helper function to create a temporary file with content for testing
fn create_temp_file(file_path: String, content: &[u8]) -> String {
    let mut file = fs::File::create(&file_path).expect("Failed to create file");
    file.write_all(content).expect("Failed to write to file");
    file_path.to_string()
}

#[test]
fn test_squashfs() {
    // Prepare test folder
    let directory = "/tmp/squashfs".to_string();
    fs::create_dir_all(Path::new(&directory.clone())).expect("Failed to create base directory");
    let file1_path = create_temp_file(format!("{}/test1.txt", directory.clone()), b"Hello, World!");
    let file2_path = create_temp_file(format!("{}/test2.txt", directory.clone()), b"Rust testing!");

    let files = vec![file1_path.clone(), file2_path.clone()];
    let output_path = "/tmp/test_archive.squashfs".to_string();

    // Create archive
    create_archive(directory.to_string(), output_path.clone())
        .expect("Failed to create archive");

    // Check if the output file exists
    assert!(fs::metadata(output_path.clone()).is_ok());

    // List files
    let listed_files = list_files(output_path.clone()).expect("Failed to list files");

    // Ensure that the files are listed correctly
    for file in files.clone() {
        assert!(listed_files.contains(&file.replace(&directory, "")));
    }

    // Read file from the archive
    let file_path = file1_path.replace(&directory, "");
    let read_content = read_file(output_path.clone(), file_path.clone())
        .expect("Failed to read file")
        .expect("File not found");
    assert_eq!(read_content, b"Hello, World!");

    // Cleanup`
    for file in files {
        fs::remove_file(file).unwrap();
    }
    fs::remove_file(output_path).unwrap();
}
