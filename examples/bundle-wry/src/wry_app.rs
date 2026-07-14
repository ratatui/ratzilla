use std::{
    error::Error,
    fs, io,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "windows")]
use tao::platform::windows::{EventLoopBuilderExtWindows, WindowExtWindows};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Window, WindowBuilder},
};
use wry::WebViewBuilder;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
use wry::WebViewBuilderExtUnix;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 8080;

enum UserEvent {
    MenuEvent(MenuEvent),
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let app_source = app_source()?;

    let mut event_loop_builder = EventLoopBuilder::<UserEvent>::with_user_event();

    let event_loop = event_loop_builder.build();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));

    let menu_bar = Menu::new();
    let app_menu = Submenu::new("bundle-wry", true);
    let about_item = MenuItem::new("About bundle-wry", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    app_menu
        .append_items(&[&about_item, &PredefinedMenuItem::separator(), &quit_item])
        .unwrap();
    menu_bar.append(&app_menu).unwrap();

    let window = WindowBuilder::new()
        .with_title("bundlle-wry")
        .build(&event_loop)?;

    #[cfg(target_os = "windows")]
    unsafe {
        menu_bar.init_for_hwnd(window.hwnd() as _).unwrap();
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        menu_bar
            .init_for_gtk_window(window.gtk_window(), window.default_vbox())
            .unwrap();
    }
    #[cfg(target_os = "macos")]
    {
        menu_bar.init_for_nsapp();
    }

    let _webview = create_webview(&window)?
        .with_url(&app_source.url)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => stop_server(&app_source.server, control_flow),
            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                if event.id() == about_item.id() {
                    println!("bundle-wry: native About menu selected");
                } else if event.id() == quit_item.id() {
                    stop_server(&app_source.server, control_flow);
                }
            }
            _ => {}
        }
    });
}

fn stop_server(server: &Arc<Mutex<Option<ServerHandle>>>, control_flow: &mut ControlFlow) {
    if let Some(handle) = server.lock().ok().and_then(|mut slot| slot.take()) {
        match handle {
            ServerHandle::Trunk(mut child) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            ServerHandle::Bundled(server) => {
                server.stop();
            }
        }
    }
    *control_flow = ControlFlow::Exit;
}

fn spawn_trunk_serve() -> Result<Child, io::Error> {
    Command::new("trunk")
        .args([
            "serve",
            "--address",
            HOST,
            "--port",
            "8080",
            "--no-autoreload",
            "true",
            "--open",
            "false",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

fn wait_for_server() -> Result<(), io::Error> {
    let addr: SocketAddr = format!("{HOST}:{PORT}").parse().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid address: {err}"),
        )
    })?;

    for _ in 0..120 {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "trunk serve did not start on 127.0.0.1:8080",
    ))
}

enum ServerHandle {
    Trunk(Child),
    Bundled(BundledServer),
}

struct BundledServer {
    shutdown: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl BundledServer {
    fn stop(self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.lock().ok().and_then(|mut slot| slot.take()) {
            let _ = join.join();
        }
    }
}

struct AppSource {
    url: String,
    server: Arc<Mutex<Option<ServerHandle>>>,
}

fn app_source() -> Result<AppSource, Box<dyn Error>> {
    if let Some(index) = bundled_index_path()? {
        let server = spawn_bundled_server(index.parent().unwrap().to_path_buf())?;
        return Ok(AppSource {
            url: format!("http://{HOST}:{PORT}"),
            server: Arc::new(Mutex::new(Some(ServerHandle::Bundled(server)))),
        });
    }

    let server = Arc::new(Mutex::new(Some(ServerHandle::Trunk(spawn_trunk_serve()?))));
    wait_for_server()?;

    Ok(AppSource {
        url: format!("http://{HOST}:{PORT}"),
        server,
    })
}

fn bundled_index_path() -> Result<Option<PathBuf>, io::Error> {
    let exe = std::env::current_exe()?;
    let Some(contents_dir) = exe.parent().and_then(|p| p.parent()) else {
        return Ok(None);
    };
    let index = contents_dir
        .join("Resources")
        .join("dist")
        .join("index.html");
    Ok(index.exists().then_some(index))
}

fn spawn_bundled_server(dist_root: PathBuf) -> Result<BundledServer, io::Error> {
    let listener = std::net::TcpListener::bind((HOST, PORT))?;
    listener.set_nonblocking(true)?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = Arc::clone(&shutdown);
    let join = thread::spawn(move || {
        while !shutdown_flag.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = handle_http_request(stream, &dist_root);
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
    });

    Ok(BundledServer {
        shutdown,
        join: Mutex::new(Some(join)),
    })
}

fn handle_http_request(
    mut stream: std::net::TcpStream,
    dist_root: &PathBuf,
) -> Result<(), io::Error> {
    stream.set_nonblocking(false)?;
    let mut buffer = [0_u8; 4096];
    let bytes_read = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let file_path = match path.split('?').next().unwrap_or("/") {
        "/" => dist_root.join("index.html"),
        other => safe_dist_path(dist_root, other),
    };

    if file_path.is_file() {
        let body = fs::read(&file_path)?;
        let content_type = content_type(&file_path);
        write_response(&mut stream, "200 OK", content_type, &body)?;
    } else {
        write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found",
        )?;
    }

    Ok(())
}

fn safe_dist_path(dist_root: &PathBuf, request_path: &str) -> PathBuf {
    let mut path = PathBuf::new();
    for component in Path::new(request_path).components() {
        use std::path::Component;
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir | Component::RootDir => {}
            _ => return dist_root.join("__invalid__"),
        }
    }
    dist_root.join(path)
}

fn content_type(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

fn write_response(
    stream: &mut std::net::TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), io::Error> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn create_webview(_window: &Window) -> Result<wry::WebViewBuilder<'_>, wry::Error> {
    Ok(WebViewBuilder::new_gtk(window.default_vbox().unwrap()))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
fn create_webview(_window: &Window) -> Result<wry::WebViewBuilder<'_>, wry::Error> {
    Ok(WebViewBuilder::new())
}
