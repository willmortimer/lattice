//! Finder “Add folder to Lattice” (NSServices) and shared service-item parsing.
//!
//! The menu item is declared in `Info.plist`. Folder paths are classified with
//! the same [`crate::open_file::open_payload_for_file`] path as `RunEvent::Opened`.

use std::path::{Path, PathBuf};

/// Parse a Finder service / pasteboard item into a local path.
///
/// Accepts ordinary paths and `file://` URLs. Empty strings are ignored.
pub fn path_from_service_item(item: &str) -> Option<PathBuf> {
    let trimmed = item.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = url::Url::parse(trimmed) {
        if url.scheme() == "file" {
            return url.to_file_path().ok();
        }
    }
    Some(PathBuf::from(trimmed))
}

pub fn paths_from_service_items<I, S>(items: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    items
        .into_iter()
        .filter_map(|item| path_from_service_item(item.as_ref()))
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn install(_app: &tauri::AppHandle) {}

#[cfg(target_os = "macos")]
pub fn install(app: &tauri::AppHandle) {
    macos::install(app);
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{define_class, msg_send, ClassType, MainThreadOnly};
    use objc2_app_kit::{
        NSApplication, NSPasteboard, NSPasteboardTypeFileURL, NSUpdateDynamicServices,
    };
    use objc2_foundation::{MainThreadMarker, NSArray, NSObject, NSString, NSURL};
    use tauri::AppHandle;

    use super::path_from_service_item;

    static SERVICE_APP: Mutex<Option<AppHandle>> = Mutex::new(None);

    define_class!(
        // SAFETY: NSObject has no subclassing requirements; this type has no Drop.
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[name = "LatticeAddFolderService"]
        struct LatticeAddFolderService;

        impl LatticeAddFolderService {
            // NSMessage `addFolderToLattice` → `addFolderToLattice:userData:error:`.
            #[unsafe(method(addFolderToLattice:userData:error:))]
            fn add_folder_to_lattice(
                &self,
                pboard: &NSPasteboard,
                _user_data: Option<&NSString>,
                _error: *mut *mut NSString,
            ) {
                let paths = paths_from_pasteboard(pboard);
                dispatch_opened_paths(&paths);
            }
        }
    );

    impl LatticeAddFolderService {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            unsafe { msg_send![Self::alloc(mtm), init] }
        }
    }

    pub(super) fn install(app: &AppHandle) {
        if let Ok(mut slot) = SERVICE_APP.lock() {
            *slot = Some(app.clone());
        }
        let Some(mtm) = MainThreadMarker::new() else {
            eprintln!("lattice: Finder add-folder service skipped (not on main thread)");
            return;
        };
        let provider = LatticeAddFolderService::new(mtm);
        let ns_app = NSApplication::sharedApplication(mtm);
        let provider_ref: &AnyObject = provider.as_ref();
        unsafe { ns_app.setServicesProvider(Some(provider_ref)) };
        NSUpdateDynamicServices();
        // NSApplication retains the provider; keep our retain for process lifetime.
        std::mem::forget(provider);
    }

    fn dispatch_opened_paths(paths: &[PathBuf]) {
        let app = {
            let Ok(guard) = SERVICE_APP.lock() else {
                return;
            };
            guard.clone()
        };
        let Some(app) = app else {
            return;
        };
        for path in paths {
            crate::dispatch_opened_path(&app, path);
        }
    }

    fn paths_from_pasteboard(pboard: &NSPasteboard) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        if let Some(url_string) = pboard.stringForType(unsafe { NSPasteboardTypeFileURL }) {
            if let Some(path) = path_from_service_item(&url_string.to_string()) {
                paths.push(path);
            }
        }

        if paths.is_empty() {
            let classes = NSArray::from_slice(&[NSURL::class()]);
            if let Some(objects) = unsafe { pboard.readObjectsForClasses_options(&classes, None) } {
                for obj in objects {
                    if let Ok(url) = obj.downcast::<NSURL>() {
                        if url.isFileURL() {
                            if let Some(ns_path) = url.path() {
                                if let Some(path) = path_from_service_item(&ns_path.to_string()) {
                                    paths.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }

        if paths.is_empty() {
            #[allow(deprecated)]
            let filenames_type = unsafe { objc2_app_kit::NSFilenamesPboardType };
            if let Some(plist) = pboard.propertyListForType(filenames_type) {
                if let Ok(array) = plist.downcast::<NSArray>() {
                    for obj in array {
                        if let Ok(name) = obj.downcast::<NSString>() {
                            if let Some(path) = path_from_service_item(&name.to_string()) {
                                paths.push(path);
                            }
                        }
                    }
                }
            }
        }

        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_folder_path() {
        let path = path_from_service_item("/Users/me/Notes").unwrap();
        assert_eq!(path, Path::new("/Users/me/Notes"));
    }

    #[cfg(unix)]
    #[test]
    fn parses_file_url() {
        let path = path_from_service_item("file:///Users/me/My%20Folder").unwrap();
        assert_eq!(path, Path::new("/Users/me/My Folder"));
    }

    #[test]
    fn ignores_empty_and_keeps_order() {
        assert_eq!(
            paths_from_service_items(["", "/tmp/a", "/tmp/b"]),
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]
        );
    }
}
