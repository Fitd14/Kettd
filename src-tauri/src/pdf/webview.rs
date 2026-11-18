//! 离屏 WebView2 打印。只在 Windows 编译；非 Windows 直接返回可读错误。
//!
//! 结构：`print_html` 起一条独立线程做全部 COM 工作（WebView2 要求 STA + 消息泵），
//! 主调用方用带超时的通道等它，避免把 Tauri 的命令线程卷进消息循环。

use std::path::Path;

#[cfg(not(windows))]
pub(crate) fn print_html(_html: &str, _user_data_dir: &Path, _output: &Path) -> Result<(), String> {
    Err("当前系统不支持导出 PDF".to_string())
}

#[cfg(windows)]
pub(crate) fn print_html(html: &str, user_data_dir: &Path, output: &Path) -> Result<(), String> {
    imp::print_html(html, user_data_dir, output)
}

#[cfg(windows)]
mod imp {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2Controller, ICoreWebView2Environment,
        ICoreWebView2Environment6, ICoreWebView2PrintSettings, ICoreWebView2_7,
    };
    use webview2_com::{
        CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
        NavigationCompletedEventHandler, PrintToPdfCompletedHandler,
    };
    use windows::core::{Interface, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{
        GetLastError, E_POINTER, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, RECT,
        RPC_E_CHANGED_MODE, WPARAM,
    };
    use windows::Win32::Graphics::Gdi::UpdateWindow;
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, IsWindow, PeekMessageW,
        RegisterClassW, ShowWindow, TranslateMessage, MSG, PM_REMOVE, SW_SHOWNOACTIVATE, WNDCLASSW,
        WS_OVERLAPPED,
    };

    /// A4 在 96 DPI 下的像素近似值；只影响离屏窗口大小，正式分页由打印设置决定。
    const WINDOW_W: i32 = 1240;
    const WINDOW_H: i32 = 1754;
    /// 离屏窗口放在屏幕外，用户看不到；仍是真实窗口，WebView2 才肯渲染。
    const OFFSCREEN_X: i32 = -32000;
    const OFFSCREEN_Y: i32 = -32000;

    const STEP_TIMEOUT: Duration = Duration::from_secs(40);
    const PRINT_TIMEOUT: Duration = Duration::from_secs(30);

    struct Window(HWND);

    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn create_window() -> Result<Window, String> {
        let class_name = HSTRING::from("kettd-pdf-export");
        let title = HSTRING::from("kettd pdf export");
        let instance =
            unsafe { GetModuleHandleW(None) }.map_err(|_| "初始化打印窗口失败".to_string())?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hInstance: instance.into(),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            // 同一进程里第二次导出会重复注册，这个错误可以忽略。
            let code = unsafe { GetLastError() };
            if code != ERROR_CLASS_ALREADY_EXISTS {
                return Err("初始化打印窗口失败".to_string());
            }
        }
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPED,
                OFFSCREEN_X,
                OFFSCREEN_Y,
                WINDOW_W,
                WINDOW_H,
                None,
                None,
                None,
                None,
            )
        }
        .map_err(|_| "创建打印窗口失败".to_string())?;
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return Err("创建打印窗口失败".to_string());
        }
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = UpdateWindow(hwnd);
        }
        Ok(Window(hwnd))
    }

    fn pump_once() {
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn wait_for<T>(rx: &mpsc::Receiver<T>, timeout: Duration, what: &str) -> Result<T, String> {
        let started = Instant::now();
        loop {
            pump_once();
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(value) => return Ok(value),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if started.elapsed() > timeout {
                        return Err(what.to_string());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("打印进程意外中断".to_string());
                }
            }
        }
    }

    fn create_environment(
        user_data_dir: &Path,
        hwnd: HWND,
    ) -> Result<(ICoreWebView2Environment, ICoreWebView2Controller), String> {
        std::fs::create_dir_all(user_data_dir)
            .map_err(|_| "无法准备打印所需的临时目录".to_string())?;
        let user_data = HSTRING::from(user_data_dir.to_string_lossy().to_string());

        let (env_tx, env_rx) = mpsc::channel();
        let env_user_data = user_data.clone();
        let _env_token = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                CreateCoreWebView2EnvironmentWithOptions(
                    None,
                    PCWSTR(env_user_data.as_ptr()),
                    None,
                    &handler,
                )
                .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, environment| {
                error_code?;
                let environment =
                    environment.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                let _ = env_tx.send(environment);
                Ok(())
            }),
        )
        .map_err(|_| "这台电脑缺少导出 PDF 所需的运行组件".to_string())?;
        let environment = wait_for(&env_rx, STEP_TIMEOUT, "准备打印组件超时，请稍后重试")?;

        let (ctl_tx, ctl_rx) = mpsc::channel();
        let env_for_ctl = environment.clone();
        let _ctl_token = CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                env_for_ctl
                    .CreateCoreWebView2Controller(hwnd, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |error_code, controller| {
                error_code?;
                let controller = controller.ok_or_else(|| windows::core::Error::from(E_POINTER))?;
                let _ = ctl_tx.send(controller);
                Ok(())
            }),
        )
        .map_err(|_| "这台电脑缺少导出 PDF 所需的运行组件".to_string())?;
        let controller = wait_for(&ctl_rx, STEP_TIMEOUT, "准备打印组件超时，请稍后重试")?;
        Ok((environment, controller))
    }

    /// 打印设置：A4 纸张、打印背景色（主题纸面）、不要页眉页脚。
    /// 拿不到 `ICoreWebView2Environment6` 时返回 None，退回浏览器默认设置。
    fn print_settings(environment: &ICoreWebView2Environment) -> Option<ICoreWebView2PrintSettings> {
        let environment6: ICoreWebView2Environment6 = environment.cast().ok()?;
        unsafe {
            let settings = environment6.CreatePrintSettings().ok()?;
            let _ = settings.SetPageWidth(8.27);
            let _ = settings.SetPageHeight(11.69);
            let _ = settings.SetShouldPrintBackgrounds(true);
            let _ = settings.SetShouldPrintHeaderAndFooter(false);
            Some(settings)
        }
    }

    fn print_html_inner(html: &str, user_data_dir: &Path, output: &Path) -> Result<(), String> {
        let co = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok();
        let com_ready = match co {
            Ok(()) => true,
            Err(err) if err.code() == RPC_E_CHANGED_MODE => false,
            Err(_) => return Err("这台电脑缺少导出 PDF 所需的运行组件".to_string()),
        };

        let result = (|| -> Result<(), String> {
            let window = create_window()?;
            let (environment, controller) = create_environment(user_data_dir, window.0)?;
            let bounds = RECT {
                left: 0,
                top: 0,
                right: WINDOW_W,
                bottom: WINDOW_H,
            };
            unsafe {
                controller
                    .SetBounds(bounds)
                    .map_err(|_| "创建打印窗口失败".to_string())?;
                controller
                    .SetIsVisible(true)
                    .map_err(|_| "创建打印窗口失败".to_string())?;
            }
            let webview = unsafe { controller.CoreWebView2() }
                .map_err(|_| "这台电脑缺少导出 PDF 所需的运行组件".to_string())?;

            let (nav_tx, nav_rx) = mpsc::channel();
            let mut nav_token = 0;
            unsafe {
                webview
                    .add_NavigationCompleted(
                        &NavigationCompletedEventHandler::create(Box::new(move |_sender, _args| {
                            let _ = nav_tx.send(());
                            Ok(())
                        })),
                        &mut nav_token,
                    )
                    .map_err(|_| "加载文档内容失败".to_string())?;
                let html = HSTRING::from(html);
                webview
                    .NavigateToString(PCWSTR(html.as_ptr()))
                    .map_err(|_| "加载文档内容失败".to_string())?;
            }
            wait_for(&nav_rx, STEP_TIMEOUT, "加载文档内容超时")?;
            // 导航完成到布局稳定之间还有一帧，图片与字体需要这点时间才参与排版。
            for _ in 0..10 {
                pump_once();
                std::thread::sleep(Duration::from_millis(20));
            }

            let webview7: ICoreWebView2_7 = webview
                .cast()
                .map_err(|_| "这台电脑的浏览器组件太旧，无法导出 PDF".to_string())?;
            let settings = print_settings(&environment);

            let (pdf_tx, pdf_rx) = mpsc::channel();
            let output_path = HSTRING::from(output.to_string_lossy().to_string());
            unsafe {
                webview7
                    .PrintToPdf(
                        PCWSTR(output_path.as_ptr()),
                        settings.as_ref(),
                        &PrintToPdfCompletedHandler::create(Box::new(
                            move |error_code, success| {
                                let _ = pdf_tx.send((error_code.is_ok(), success));
                                Ok(())
                            },
                        )),
                    )
                    .map_err(|_| "发起打印失败".to_string())?;
            }
            let (call_ok, printed) = wait_for(&pdf_rx, PRINT_TIMEOUT, "打印超时，请稍后重试")?;
            if !call_ok || !printed {
                return Err("打印没有完成，请重试".to_string());
            }

            unsafe {
                let _ = controller.Close();
            }
            drop(window);
            Ok(())
        })();

        if com_ready {
            unsafe { CoUninitialize() };
        }
        result
    }

    pub(super) fn print_html(
        html: &str,
        user_data_dir: &Path,
        output: &Path,
    ) -> Result<(), String> {
        let user_data: PathBuf = user_data_dir.to_path_buf();
        let output_path: PathBuf = output.to_path_buf();
        let html = html.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = print_html_inner(&html, &user_data, &output_path);
            let _ = tx.send(result);
        });
        // 内层每一步都有自己的超时，这里再兜一层，防止线程卡死把命令挂住。
        match rx.recv_timeout(STEP_TIMEOUT + PRINT_TIMEOUT) {
            Ok(result) => result,
            Err(_) => Err("打印超时，请稍后重试".to_string()),
        }
    }
}

