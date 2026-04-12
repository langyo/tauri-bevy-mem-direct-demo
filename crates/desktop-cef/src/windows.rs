use cef::{args::Args, *};
use shared::shm::{ShmHandle, SHM_MAX_SIZE, SHM_NAME};
use std::{
    cell::RefCell,
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Default)]
struct DemoClientState {
    browser_count: usize,
}

static RENDER_SHM: OnceLock<Mutex<Option<ShmHandle>>> = OnceLock::new();

fn with_render_shm<T>(f: impl FnOnce(&ShmHandle) -> T) -> Option<T> {
    let slot = RENDER_SHM.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().ok()?;
    if guard.is_none() {
        match ShmHandle::open(SHM_NAME, SHM_MAX_SIZE) {
            Ok(shm) => {
                *guard = Some(shm);
            }
            Err(error) => {
                tracing::error!(%error, "CEF render process failed to open shared memory");
                return None;
            }
        }
    }

    guard.as_ref().map(f)
}

wrap_v8_array_buffer_release_callback! {
    struct NoopReleaseCallback;

    impl V8ArrayBufferReleaseCallback {
        fn release_buffer(&self, _buffer: *mut u8) {}
    }
}

wrap_v8_handler! {
    struct FrameSabV8Handler;

    impl V8Handler {
        fn execute(
            &self,
            name: Option<&CefString>,
            _object: Option<&mut V8Value>,
            _arguments: Option<&[Option<V8Value>]>,
            retval: Option<&mut Option<V8Value>>,
            _exception: Option<&mut CefString>,
        ) -> ::std::os::raw::c_int {
            let method = name.map(CefString::to_string).unwrap_or_default();
            if method != "getFrameSab" {
                return 0;
            }

            let array_buffer = with_render_shm(|shm| {
                let mut callback = NoopReleaseCallback::new();
                v8_value_create_array_buffer(
                    shm.as_ptr() as *mut u8,
                    shm.size(),
                    Some(&mut callback),
                )
            })
            .flatten();

            let Some(mut array_buffer) = array_buffer else {
                tracing::error!("Failed to create V8 ArrayBuffer from shared memory");
                return 0;
            };

            if let Some(retval) = retval {
                *retval = Some(array_buffer.clone());
            }

            1
        }
    }
}

wrap_render_process_handler! {
    struct DemoRenderProcessHandler;

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if frame.is_main() != 1 {
                return;
            }

            let Some(context) = context else {
                return;
            };
            let Some(mut global) = context.global() else {
                return;
            };

            let name = CefString::from("getFrameSab");
            let mut handler = FrameSabV8Handler::new();
            let Some(mut func) = v8_value_create_function(Some(&name), Some(&mut handler)) else {
                tracing::error!("Failed to create getFrameSab V8 function");
                return;
            };

            let _ = global.set_value_bykey(
                Some(&name),
                Some(&mut func),
                V8Propertyattribute::default(),
            );

            let code = CefString::from(
                "(function(){try{if(!window.__frameSab&&typeof getFrameSab==='function'){window.__frameSab=getFrameSab();}}catch(_e){}})();",
            );
            let script_url = CefString::from("app://cef-zero-copy-bootstrap.js");
            frame.execute_java_script(Some(&code), Some(&script_url), 1);
            tracing::info!("Installed CEF zero-copy V8 bridge for __frameSab");
        }
    }
}

impl DemoClientState {
    fn on_after_created(&mut self, _browser: Option<&mut Browser>) {
        self.browser_count += 1;
    }

    fn do_close(&mut self, _browser: Option<&mut Browser>) -> bool {
        false
    }

    fn on_before_close(&mut self, _browser: Option<&mut Browser>) {
        if self.browser_count > 0 {
            self.browser_count -= 1;
        }

        if self.browser_count == 0 {
            quit_message_loop();
        }
    }

    fn on_load_error(
        &mut self,
        frame: Option<&mut Frame>,
        error_code: Errorcode,
        error_text: Option<&CefString>,
        failed_url: Option<&CefString>,
    ) {
        if let Some(frame) = frame {
            if frame.is_main() == 0 {
                return;
            }
        }

        let error_text = error_text.map(CefString::to_string).unwrap_or_default();
        let failed_url = failed_url.map(CefString::to_string).unwrap_or_default();
        tracing::error!(?error_code, %error_text, %failed_url, "CEF failed to load main frame");
    }
}

wrap_life_span_handler! {
    struct DemoLifeSpanHandler {
        inner: Arc<Mutex<DemoClientState>>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let mut inner = self.inner.lock().expect("Failed to lock CEF client state");
            inner.on_after_created(browser);
        }

        fn do_close(&self, browser: Option<&mut Browser>) -> i32 {
            let mut inner = self.inner.lock().expect("Failed to lock CEF client state");
            inner.do_close(browser).into()
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let mut inner = self.inner.lock().expect("Failed to lock CEF client state");
            inner.on_before_close(browser);
        }
    }
}

wrap_load_handler! {
    struct DemoLoadHandler {
        inner: Arc<Mutex<DemoClientState>>,
    }

    impl LoadHandler {
        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let mut inner = self.inner.lock().expect("Failed to lock CEF client state");
            inner.on_load_error(frame, error_code, error_text, failed_url);
        }
    }
}

wrap_client! {
    struct DemoClient {
        inner: Arc<Mutex<DemoClientState>>,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(DemoLifeSpanHandler::new(self.inner.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(DemoLoadHandler::new(self.inner.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct DemoBrowserProcessHandler {
        url: String,
        client: RefCell<Option<Client>>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let mut client_slot = self.client.borrow_mut();
            if client_slot.is_none() {
                *client_slot = Some(DemoClient::new(Arc::new(Mutex::new(DemoClientState::default()))));
            }

            let settings = BrowserSettings::default();
            let mut window_info = WindowInfo::default().set_as_popup(Default::default(), "demo-panel-cef");
            window_info.runtime_style = RuntimeStyle::ALLOY;
            let url = CefString::from(self.url.as_str());
            let mut client = client_slot.clone();

            let created = browser_host_create_browser(
                Some(&window_info),
                client.as_mut(),
                Some(&url),
                Some(&settings),
                None,
                None,
            );

            if created == 1 {
                tracing::info!(%self.url, "CEF browser window created");
            } else {
                tracing::error!(%self.url, created, "CEF browser window creation failed");
            }
        }
    }
}

wrap_app! {
    struct DemoAppBuilder {
        url: String,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let Some(command_line) = command_line else {
                return;
            };

            command_line.append_switch(Some(&"autoplay-policy=no-user-gesture-required".into()));
            command_line.append_switch(Some(&"disable-web-security".into()));
            command_line.append_switch(Some(&"no-v8-sandbox".into()));
            command_line.append_switch_with_value(
                Some(&"disable-features".into()),
                Some(&"V8Sandbox".into()),
            );
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(DemoBrowserProcessHandler::new(self.url.clone(), RefCell::new(None)))
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(DemoRenderProcessHandler::new())
        }
    }
}

impl DemoAppBuilder {
    fn build(url: String) -> App {
        Self::new(url)
    }
}

fn parse_cef_args() -> anyhow::Result<(Args, bool)> {
    let args = Args::new();
    let cmd_line = args
        .as_cmd_line()
        .ok_or_else(|| anyhow::anyhow!("Failed to parse command line arguments"))?;

    let switch = CefString::from("type");
    let is_browser_process = cmd_line.has_switch(Some(&switch)) != 1;

    Ok((args, is_browser_process))
}

fn run_subprocess(args: &Args) -> anyhow::Result<()> {
    let mut app = DemoAppBuilder::build(String::new());

    let ret = execute_process(Some(args.as_main_args()), Some(&mut app), ptr::null_mut());
    anyhow::ensure!(ret >= 0, "Cannot execute non-browser process: {ret}");
    Ok(())
}

fn run_browser_process(args: &Args, url: &str) -> anyhow::Result<()> {
    let mut app = DemoAppBuilder::build(url.to_string());

    let ret = execute_process(Some(args.as_main_args()), Some(&mut app), ptr::null_mut());
    anyhow::ensure!(ret == -1, "Cannot execute browser process: {ret}");

    let settings = Settings {
        no_sandbox: 1,
        ..Default::default()
    };

    anyhow::ensure!(
        initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            ptr::null_mut(),
        ) == 1,
        "CEF initialize failed"
    );

    run_message_loop();
    shutdown();
    Ok(())
}

pub fn main() {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    let (args, is_browser_process) = match parse_cef_args() {
        Ok(parsed) => parsed,
        Err(error) => panic!("{error:?}"),
    };

    if !is_browser_process {
        if let Err(error) = run_subprocess(&args) {
            panic!("{error:?}");
        }
        return;
    }

    let runtime = match crate::common::start_runtime("windows-cef") {
        Ok(runtime) => runtime,
        Err(error) => panic!("{error}"),
    };

    let result = run_browser_process(&args, runtime.url());
    runtime.shutdown();

    if let Err(error) = result {
        panic!("{error:?}");
    }
}
