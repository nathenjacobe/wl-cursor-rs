use std::io::{self, Write};
use std::os::fd::AsFd;

use tempfile::tempfile;

use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::protocol::wl_pointer::{self, WlPointer};

use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, Event as LayerSurfaceEvent, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

struct App {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,

    seat: Option<WlSeat>,
    surface: Option<WlSurface>,
    layer_surface: Option<ZwlrLayerSurfaceV1>,

    _buffer: Option<WlBuffer>,

    _pool: Option<WlShmPool>,

    cursor_x: f64,
    cursor_y: f64,
    cursor_known: bool,

    _pointer: Option<WlPointer>,

    _output: Option<WlOutput>,

    running: bool,
}

impl App {
    fn new() -> Self {
        Self {
            compositor: None,
            shm: None,
            layer_shell: None,

            seat: None,
            surface: None,
            layer_surface: None,

            _buffer: None,
            _pool: None,

            cursor_x: 0.0,
            cursor_y: 0.0,
            cursor_known: false,

            _pointer: None,
            _output: None,

            running: true,
        }
    }
}

impl Dispatch<WlRegistry, ()> for App {
    fn event(
        app: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };

        match interface.as_str() {
            "wl_compositor" => {
                app.compositor = Some(
                    registry.bind::<WlCompositor, _, _>(
                        name,
                        version.min(4),
                        qh,
                        (),
                    ),
                );
            }

            "wl_shm" => {
                app.shm = Some(
                    registry.bind::<WlShm, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    ),
                );
            }

            "wl_seat" => {
                app.seat = Some(
                    registry.bind::<WlSeat, _, _>(
                        name,
                        version.min(1),
                        qh,
                        (),
                    ),
                );
            }

            "wl_output" => {
                if app._output.is_none() {
                    app._output = Some(
                        registry.bind::<WlOutput, _, _>(
                            name,
                            version.min(4),
                            qh,
                            (),
                        ),
                    );
                }
            }

            "zwlr_layer_shell_v1" => {
                app.layer_shell = Some(
                    registry.bind::<ZwlrLayerShellV1, _, _>(
                        name,
                        version.min(2),
                        qh,
                        (),
                    ),
                );
            }

            _ => {}
        }
    }
}

impl Dispatch<WlSeat, ()> for App {
    fn event(
        app: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities { capabilities } = event else {
            return;
        };

        let WEnum::Value(capabilities) = capabilities else {
            return;
        };

        if capabilities.contains(Capability::Pointer) && app._pointer.is_none() {
            app._pointer = Some(seat.get_pointer(qh, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            } => {
                app.cursor_x = surface_x;
                app.cursor_y = surface_y;
                app.cursor_known = true;
            }

            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                app.cursor_x = surface_x;
                app.cursor_y = surface_y;
                app.cursor_known = true;
            }

            wl_pointer::Event::Button { state, .. } => {
                let WEnum::Value(state) = state else {
                    return;
                };

                if state == wl_pointer::ButtonState::Pressed && app.cursor_known {
                    println!("{:.0} {:.0}", app.cursor_x, app.cursor_y);
                    let _ = io::stdout().flush();
                }
            }

            _ => {}
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        _: &ZwlrLayerSurfaceV1,
        event: LayerSurfaceEvent,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            LayerSurfaceEvent::Configure {
                serial,
                width,
                height,
            } => {
                let Some(layer_surface) = app.layer_surface.as_ref() else {
                    app.running = false;
                    return;
                };

                layer_surface.ack_configure(serial);

                let (Some(surface), Some(shm)) = (
                    app.surface.as_ref(),
                    app.shm.as_ref(),
                ) else {
                    app.running = false;
                    return;
                };

                if width == 0 || height == 0 {
                    eprintln!(
                        "Compositor returned invalid layer surface size: {}x{}",
                        width, height
                    );
                    app.running = false;
                    return;
                }

                match create_transparent_buffer(shm, width, height, qh) {
                    Ok((pool, buffer)) => {
                        surface.attach(Some(&buffer), 0, 0);

                        surface.damage_buffer(
                            0,
                            0,
                            width as i32,
                            height as i32,
                        );

                        surface.commit();

                        app._pool = Some(pool);
                        app._buffer = Some(buffer);
                    }

                    Err(err) => {
                        eprintln!("Failed to create wl_shm buffer: {err}");
                        app.running = false;
                    }
                }
            }

            LayerSurfaceEvent::Closed => {
                app.running = false;
            }

            _ => {}
        }
    }
}

fn create_transparent_buffer(
    shm: &WlShm,
    width: u32,
    height: u32,
    qh: &QueueHandle<App>,
) -> io::Result<(WlShmPool, WlBuffer)> {
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "stride overflow"))?;

    let size = stride
        .checked_mul(height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "buffer size overflow"))?;

    let size_i32 = i32::try_from(size)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "buffer too large"))?;

    let file = tempfile()?;
    file.set_len(size as u64)?;

    let pool = shm.create_pool(
        file.as_fd(),
        size_i32,
        qh,
        (),
    );

    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );

    Ok((pool, buffer))
}

wayland_client::delegate_noop!(App: ignore WlCompositor);
wayland_client::delegate_noop!(App: ignore WlSurface);
wayland_client::delegate_noop!(App: ignore WlBuffer);
wayland_client::delegate_noop!(App: ignore WlOutput);
wayland_client::delegate_noop!(App: ignore WlShm);
wayland_client::delegate_noop!(App: ignore WlShmPool);
wayland_client::delegate_noop!(App: ignore ZwlrLayerShellV1);

fn main() {
    let connection =
        Connection::connect_to_env().expect("failed to connect to Wayland");

    let mut event_queue = connection.new_event_queue();
    let qh = event_queue.handle();

    connection.display().get_registry(&qh, ());

    let mut app = App::new();

    event_queue
        .roundtrip(&mut app)
        .expect("Wayland roundtrip failed");

    let required = [
        ("wl_compositor", app.compositor.is_some()),
        ("wl_shm", app.shm.is_some()),
        ("wl_seat", app.seat.is_some()),
        ("zwlr_layer_shell_v1", app.layer_shell.is_some()),
    ];

    for (name, present) in required {
        if !present {
            eprintln!("The compositor lacks {name} support!");
            std::process::exit(1);
        }
    }

    let surface = app
        .compositor
        .as_ref()
        .unwrap()
        .create_surface(&qh, ());

    app.surface = Some(surface.clone());

    let layer_surface = app
        .layer_shell
        .as_ref()
        .unwrap()
        .get_layer_surface(
            &surface,
            None,
            Layer::Overlay,
            "find-cursor".to_owned(),
            &qh,
            (),
        );

    app.layer_surface = Some(layer_surface.clone());

    layer_surface.set_size(0, 0);

    layer_surface.set_anchor(
        Anchor::Top
            | Anchor::Bottom
            | Anchor::Left
            | Anchor::Right,
    );

    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);

    layer_surface.set_exclusive_zone(-1);

    surface.commit();

    while app.running {
        if let Err(err) = event_queue.blocking_dispatch(&mut app) {
            eprintln!("Wayland dispatch failed: {err}");
            break;
        }
    }
}
