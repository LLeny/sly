use std::path::PathBuf;

use bevy::math::Vec4;
use bevy::window::RawHandleWrapper;
use objc2::encode::{Encode, Encoding};
use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_avf_audio::{AVAudioSession, AVAudioSessionCategoryPlayback};
use raw_window_handle::RawWindowHandle;

pub fn documents_dir() -> PathBuf {
    dirs::document_dir().unwrap_or_else(|| PathBuf::from("Documents"))
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EdgeInsets {
    top: f64,
    left: f64,
    bottom: f64,
    right: f64,
}

unsafe impl Encode for EdgeInsets {
    const ENCODING: Encoding = Encoding::Struct(
        "UIEdgeInsets",
        &[f64::ENCODING, f64::ENCODING, f64::ENCODING, f64::ENCODING],
    );
}

pub fn safe_area_insets(window: &RawHandleWrapper) -> Vec4 {
    let RawWindowHandle::UiKit(handle) = window.get_window_handle() else {
        return Vec4::ZERO;
    };

    let insets: EdgeInsets = unsafe {
        let view: *mut AnyObject = handle.ui_view.as_ptr().cast();
        msg_send![view, safeAreaInsets]
    };

    Vec4::new(
        insets.left as f32,
        insets.top as f32,
        insets.right as f32,
        insets.bottom as f32,
    )
}

pub fn start_audio_session() {
    let result = unsafe {
        let session = AVAudioSession::sharedInstance();
        AVAudioSessionCategoryPlayback
            .ok_or_else(|| "AVAudioSessionCategoryPlayback is unavailable".to_string())
            .and_then(|category| {
                session
                    .setCategory_error(category)
                    .and_then(|()| session.setActive_error(true))
                    .map_err(|err| err.localizedDescription().to_string())
            })
    };

    if let Err(err) = result {
        eprintln!("sly-launcher: audio session setup failed, sound may stay muted: {err}");
    }
}
