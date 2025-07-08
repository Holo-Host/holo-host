use super::*;
use std::fs;
use std::io::Write;

// Helper function to create a temporary file with content for testing
fn create_temp_file(name: &str, content: &[u8]) -> String {
    let path = format!("/tmp/{}", name);
    let mut file = File::create(&path).expect("Failed to create file");
    file.write_all(content).expect("Failed to write to file");
    path
}

#[test]
fn test_squashfs() {
    // Prepare test files
    let file1_path = create_temp_file("test1.txt", b"Hello, World!");
    let file2_path = create_temp_file("test2.txt", b"Rust testing!");

    let files = vec![file1_path.clone(), file2_path.clone()];
    let output_path = "/tmp/test_archive.squashfs";

    // Create archive
    create_archive(output_path.to_string(), files.clone(), "/tmp".to_string())
        .expect("Failed to create archive");

    // Check if the output file exists
    assert!(fs::metadata(output_path).is_ok());

    // List files
    let listed_files = list_files(output_path.to_string()).expect("Failed to list files");

    // Ensure that the files are listed correctly
    assert!(listed_files.contains(&file1_path.replace("/tmp", "")));
    assert!(listed_files.contains(&file2_path.replace("/tmp", "")));

    // Read file from the archive
    let file_path = file1_path.replace("/tmp", "");
    let read_content = read_file(output_path.to_string(), file_path.clone())
        .expect("Failed to read file")
        .expect("File not found");
    assert_eq!(read_content, b"Hello, World!");

    // Cleanup`
    for file in files {
        fs::remove_file(file).unwrap();
    }
    fs::remove_file(output_path).unwrap();
}
