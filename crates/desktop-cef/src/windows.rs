use cef::{args::Args, *};
use std::{
    cell::RefCell,
    ptr,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP, WS_VISIBLE};

#[derive(Default)]
struct DemoClientState {
    browser_count: usize,
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
            window_info.style = WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE;
            let url = CefString::from(self.url.as_str());
            let mut client = client_slot.clone();

            browser_host_create_browser(
                Some(&window_info),
                client.as_mut(),
                Some(&url),
                Some(&settings),
                None,
                None,
            );
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
            command_line.append_switch(Some(&"disable-gpu".into()));
            command_line.append_switch(Some(&"disable-gpu-compositing".into()));
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(DemoBrowserProcessHandler::new(self.url.clone(), RefCell::new(None)))
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
