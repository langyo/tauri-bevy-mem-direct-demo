use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WinitWindows;
use std::cell::UnsafeCell;

pub struct WryOverlayPlugin {
    pub port: u16,
}

impl Plugin for WryOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WryOverlayUrl(format!(
            "http://127.0.0.1:{}/index.html",
            self.port
        )))
        .insert_resource(WryWebViewHandle(UnsafeCell::new(None)))
        .add_systems(Update, init_webview);
    }
}

#[derive(Resource)]
struct WryOverlayUrl(String);

#[derive(Resource)]
pub struct WryWebViewHandle(UnsafeCell<Option<wry::WebView>>);

unsafe impl Send for WryWebViewHandle {}
unsafe impl Sync for WryWebViewHandle {}

impl WryWebViewHandle {
    pub fn as_ref(&self) -> Option<&wry::WebView> {
        unsafe { (*self.0.get()).as_ref() }
    }
}

fn init_webview(
    primary: Query<Entity, With<PrimaryWindow>>,
    winit_windows: NonSend<WinitWindows>,
    handle: ResMut<WryWebViewHandle>,
    config: Res<WryOverlayUrl>,
) {
    if unsafe { (*handle.0.get()).is_some() } {
        return;
    }
    let entity = primary.single();
    let Some(wrapper) = winit_windows.get_window(entity) else {
        return;
    };
    let winit: &winit::window::Window = &**wrapper;

    let webview = wry::WebViewBuilder::new()
        .with_url(&config.0)
        .with_transparent(true)
        .with_devtools(true)
        .build(winit)
        .expect("failed to build wry webview");

    unsafe {
        *handle.0.get() = Some(webview);
    }
    info!("wry overlay webview created at {}", config.0);
}
