//! Where captured pixels should be routed after capture.

/// Post-capture routing target (handled above the backend in desktop flows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureDestination {
    Clipboard,
    CaptureInbox,
    CurrentNote,
    CurrentCanvas,
    NamedCollection(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_collection_stores_arbitrary_label() {
        let destination = CaptureDestination::NamedCollection("Field notes".into());
        assert_eq!(
            destination,
            CaptureDestination::NamedCollection("Field notes".into())
        );
    }

    #[test]
    fn clipboard_and_inbox_are_distinct() {
        assert_ne!(
            CaptureDestination::Clipboard,
            CaptureDestination::CaptureInbox
        );
        assert_ne!(
            CaptureDestination::CurrentNote,
            CaptureDestination::CurrentCanvas
        );
    }
}
