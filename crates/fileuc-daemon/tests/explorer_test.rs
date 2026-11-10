#[test]
fn test_get_selected_files() {
    let files = fileuc_daemon::explorer::get_selected_files();
    println!("Selected files: {:?}", files);
}
