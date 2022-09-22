use std::{
    sync::{
        mpsc::{self, TryRecvError},
        Arc, Mutex,
    },
    time::Duration,
};

use fltk::{
    app, button::Button, enums::Align, frame::Frame, group::Flex, input::IntInput, prelude::*,
    window::Window,
};

fn simulate(event_type: &rdev::EventType) {
    match rdev::simulate(event_type) {
        Ok(()) => (),
        Err(rdev::SimulateError) => {
            eprintln!("Could not send {:?}", event_type);
        }
    }

    // Make sure OS registers event
    std::thread::sleep(Duration::from_millis(10));
}

fn start_clicking(rx: Arc<Mutex<mpsc::Receiver<()>>>, delay: u64) {
    println!("Start clicking");
    std::thread::spawn(move || loop {
        simulate(&rdev::EventType::ButtonPress(rdev::Button::Left));
        simulate(&rdev::EventType::ButtonRelease(rdev::Button::Left));
        std::thread::sleep(Duration::from_millis(delay));

        match rx.lock().unwrap().try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => {
                println!("Stop clicking");
                break;
            }
            Err(TryRecvError::Empty) => (),
        }
    });
}

fn handle_rdev_event(event: &rdev::Event, s: &app::Sender<Message>) {
    match event.event_type {
        rdev::EventType::KeyPress(key) => s.send(Message::KeyPress(key)),
        _ => (),
    };
}

fn with_label<W: WidgetExt>(func: fn() -> W, label: &str) -> W {
    let flex = Flex::default();
    Frame::default()
        .with_label(label)
        .with_align(Align::Inside | Align::Right);
    let widget = func();
    flex.end();
    widget
}

#[derive(Clone, Copy)]
enum Message {
    Toggle,
    KeyPress(rdev::Key),
    StartSetKeybind,
    SetKeybind(rdev::Key),
}

fn main() {
    let app = app::App::default().with_scheme(app::Scheme::Gtk);
    let mut window = Window::default()
        .with_size(200, 300)
        .with_label("Auto clicker")
        .center_screen();

    // Layout
    let flex = Flex::default()
        .with_size(100, 100)
        .column()
        .center_of_parent();

    Frame::default().with_label("Auto clicker");

    let mut delay_ipt = with_label(IntInput::default, "Delay: ");
    delay_ipt.set_value("20");

    let mut keybind_btn = with_label(Button::default, "Keybind: ");

    flex.end();

    window.make_resizable(true);
    window.end();
    window.show();

    // Logic
    let (s, r) = app::channel::<Message>();

    // When loop_tx sends anything, the loop thread will stop
    let (loop_tx, loop_rx) = mpsc::channel();
    let rx = Arc::new(Mutex::new(loop_rx));

    std::thread::spawn(move || {
        rdev::listen(move |event| handle_rdev_event(&event, &s)).unwrap();
    });

    let mut is_clicking = false;
    let mut keybind = None;
    let mut is_setting_keybind = false;

    keybind_btn.set_callback(move |_| {
        s.send(Message::StartSetKeybind);
    });

    s.send(Message::SetKeybind(rdev::Key::F9));

    while app.wait() {
        if let Some(msg) = r.recv() {
            match msg {
                Message::Toggle => {
                    if is_clicking {
                        loop_tx.send(()).unwrap();
                        is_clicking = false;
                    } else {
                        start_clicking(rx.clone(), delay_ipt.value().parse().unwrap());
                        is_clicking = true;
                    }
                }
                Message::KeyPress(key) => {
                    if is_setting_keybind {
                        s.send(Message::SetKeybind(key));
                        is_setting_keybind = false;
                    } else if keybind.map_or(false, |keybind| keybind == key) {
                        s.send(Message::Toggle);
                    }
                }
                Message::SetKeybind(key) => {
                    keybind = Some(key);
                    keybind_btn.set_label(&format!("{key:?}"));
                }
                Message::StartSetKeybind => is_setting_keybind = true,
            }
        }
    }
}
