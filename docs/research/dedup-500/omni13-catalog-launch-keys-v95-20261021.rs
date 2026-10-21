pub struct CatalogEntry {
    pub id: String,
    pub control_name: String,
    pub breadcrumb_path: String,
    pub launch_command: String,
    pub source: String,
    pub description: String,
    pub synonyms: String,
}

fn image_path_dedupe_key(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp"
    ) {
        return None;
    }
    Some(format!(
        "image:{}",
        path.trim().replace('/', "\\").to_ascii_lowercase()
    ))
}

fn result_launch_dedupe_key(entry: &CatalogEntry) -> Option<String> {
    if entry.launch_command.is_empty() {
        return None;
    }
    if let Some(path) = entry.launch_command.strip_prefix("copy_image:") {
        return image_path_dedupe_key(path);
    }
    image_path_dedupe_key(&entry.launch_command)
        .or_else(|| Some(format!("cmd:{}", entry.launch_command)))
}


#[test]
fn private_and_public_discovery_share_launch_key(){let a=CatalogEntry { description: "Allows your PC to find and be found by other devices on private networks.".to_string(), breadcrumb_path: "Network & internet > Advanced network settings > Advanced sharing settings > Private networks > Network discovery".to_string(), control_name: "Network discovery".to_string(), launch_command: "ms-settings:network-advancedsharing".to_string(), source: "Vision".to_string(), synonyms: "find devices|discover pc|network visibility|private network discovery".to_string(), id: "network-internet.advanced-network.private-network-discovery".to_string() };let b=CatalogEntry { description: "Allows your PC to find and be found by other devices on public networks.".to_string(), breadcrumb_path: "Network & internet > Advanced network settings > Advanced sharing settings > Public networks > Network discovery".to_string(), control_name: "Network discovery".to_string(), launch_command: "ms-settings:network-advancedsharing".to_string(), source: "Vision".to_string(), synonyms: "public network visibility|find devices public|public discovery".to_string(), id: "network-internet.advanced-network.public-network-discovery".to_string() };assert_ne!(a.id,b.id);assert_ne!(a.breadcrumb_path,b.breadcrumb_path);assert_eq!(result_launch_dedupe_key(&a),result_launch_dedupe_key(&b));}
#[test]
fn private_and_public_sharing_share_launch_key(){let a=CatalogEntry { description: "Allows others on the private network to access shared files and printers on this device.".to_string(), breadcrumb_path: "Network & internet > Advanced network settings > Advanced sharing settings > Private networks > File and printer sharing".to_string(), control_name: "File and printer sharing".to_string(), launch_command: "ms-settings:network-advancedsharing".to_string(), source: "Vision".to_string(), synonyms: "share files|share printer|network sharing|private sharing".to_string(), id: "network-internet.advanced-network.private-file-printer-sharing".to_string() };let b=CatalogEntry { description: "Allows others on the public network to access shared files and printers on this device.".to_string(), breadcrumb_path: "Network & internet > Advanced network settings > Advanced sharing settings > Public networks > File and printer sharing".to_string(), control_name: "File and printer sharing".to_string(), launch_command: "ms-settings:network-advancedsharing".to_string(), source: "Vision".to_string(), synonyms: "public file share|public printer share|network sharing public".to_string(), id: "network-internet.advanced-network.public-file-printer-sharing".to_string() };assert_ne!(a.id,b.id);assert_ne!(a.breadcrumb_path,b.breadcrumb_path);assert_eq!(result_launch_dedupe_key(&a),result_launch_dedupe_key(&b));}
#[test]
fn taskbar_section_and_search_item_share_launch_key(){let a=CatalogEntry { description: "Show or hide buttons that appear on the taskbar.".to_string(), breadcrumb_path: "Personalization > Taskbar > Taskbar items".to_string(), control_name: "Taskbar items".to_string(), launch_command: "ms-settings:personalization-taskbar".to_string(), source: "Vision".to_string(), synonyms: "taskbar icons|show hide buttons|taskbar buttons".to_string(), id: "personalization.taskbar.taskbar-items".to_string() };let b=CatalogEntry { description: "Controls the visibility of the Search box or icon on the taskbar.".to_string(), breadcrumb_path: "Personalization > Taskbar > Taskbar items > Search".to_string(), control_name: "Search".to_string(), launch_command: "ms-settings:personalization-taskbar".to_string(), source: "Vision".to_string(), synonyms: "search box|search icon|taskbar search|hide search".to_string(), id: "personalization.taskbar.search".to_string() };assert_ne!(a.id,b.id);assert_ne!(a.breadcrumb_path,b.breadcrumb_path);assert_eq!(result_launch_dedupe_key(&a),result_launch_dedupe_key(&b));}
