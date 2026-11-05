//! GUI-thread native selection. Database bytes are read only by the runtime.
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use tablecraft_ui::Localizer;

type Selection = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

#[derive(Default)]
pub(super) struct FileDialog {
    pending: Option<Pending>,
}

struct Pending {
    selection: Selection,
    waker: Waker,
}

impl Pending {
    fn new(selection: Selection, context: &egui::Context) -> Self {
        let repaint = Arc::new(Repaint(context.clone()));
        Self {
            selection,
            waker: Waker::from(repaint),
        }
    }
}

struct Repaint(egui::Context);

impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}

impl FileDialog {
    pub(super) fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Create the parented sheet on the GUI thread. No nested blocking event
    /// loop, executor or second database worker is introduced by the host.
    pub(super) fn open(
        &mut self,
        frame: &eframe::Frame,
        context: &egui::Context,
        localizer: &Localizer,
    ) {
        if self.is_open() {
            return;
        }
        let selection = rfd::AsyncFileDialog::new()
            .set_parent(frame)
            .set_title(localizer.text("backstage-open-database"))
            .add_filter(
                localizer.text("shell-local-databases"),
                &["accdb", "mdb", "accde", "mde", "accdr", "accdc"],
            )
            .pick_file();
        let selection =
            Box::pin(async move { selection.await.map(|file| file.path().to_path_buf()) });
        self.pending = Some(Pending::new(selection, context));
        context.request_repaint();
    }

    /// The outer Option means completion; the inner None means no selection.
    /// A native cancellation does not dispatch any database command.
    pub(super) fn poll(&mut self) -> Option<Option<PathBuf>> {
        let pending = self.pending.as_mut()?;
        let mut context = Context::from_waker(&pending.waker);
        match pending.selection.as_mut().poll(&mut context) {
            Poll::Pending => None,
            Poll::Ready(selection) => {
                self.pending = None;
                Some(selection)
            }
        }
    }
}

/// The display field may be lossy; the selected filesystem identity is not.
pub(super) struct SelectedFile {
    path: PathBuf,
    display: String,
}

impl SelectedFile {
    pub(super) fn new(path: PathBuf) -> Self {
        let display = path.to_string_lossy().into_owned();
        Self { path, display }
    }

    pub(super) fn display(&self) -> &str {
        &self.display
    }

    pub(super) fn path_for(&self, field: &str) -> Option<&Path> {
        (field == self.display).then_some(self.path.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::{FileDialog, Pending, SelectedFile};
    use std::{
        future::{poll_fn, ready},
        path::PathBuf,
        task::Poll,
    };

    #[test]
    fn a_pending_selection_is_polled_once_per_frame_and_delivered_once() {
        let context = egui::Context::default();
        let path = PathBuf::from("chosen.accdb");
        let expected = path.clone();
        let mut first = true;
        let future = poll_fn(move |context| {
            if first {
                first = false;
                context.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(Some(path.clone()))
            }
        });
        let mut dialog = FileDialog {
            pending: Some(Pending::new(Box::pin(future), &context)),
        };
        assert!(dialog.is_open());
        assert_eq!(dialog.poll(), None);
        assert!(dialog.is_open());
        assert_eq!(dialog.poll(), Some(Some(expected)));
        assert!(!dialog.is_open());
        assert_eq!(dialog.poll(), None);
    }

    #[test]
    fn cancelled_selection_completes_once_without_a_path() {
        let context = egui::Context::default();
        let mut dialog = FileDialog {
            pending: Some(Pending::new(Box::pin(ready(None)), &context)),
        };
        assert_eq!(dialog.poll(), Some(None));
        assert!(!dialog.is_open());
        assert_eq!(dialog.poll(), None);
    }

    #[cfg(unix)]
    #[test]
    fn selected_non_unicode_path_does_not_roundtrip_through_display_text() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let path = PathBuf::from(OsString::from_vec(b"native-\xff.accdb".to_vec()));
        let selection = SelectedFile::new(path.clone());
        assert_eq!(
            selection.path_for(selection.display()),
            Some(path.as_path())
        );
        assert_ne!(PathBuf::from(selection.display()), path);
        assert_eq!(selection.path_for("edited.accdb"), None);
    }
}
