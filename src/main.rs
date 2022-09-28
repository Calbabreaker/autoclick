use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use fltk::{
    app, button::Button, enums::Align, frame::Frame, group::Flex, input::IntInput, menu::Choice,
    prelude::*, window::Window,
};

fn simulate(event_type: &rdev::EventType) {
    match rdev::simulate(event_type) {
        Ok(()) => (),
        Err(rdev::SimulateError) => {
            eprintln!("Could not send {:?}", event_type);
        }
    }

    // Make sure OS registers event
    std::thread::sleep(Duration::from_millis(5));
}

fn start_clicking(rx: Arc<Mutex<mpsc::Receiver<()>>>, delay: u64, button: rdev::Button) {
    use mpsc::TryRecvError;

    std::thread::spawn(move || loop {
        simulate(&rdev::EventType::ButtonPress(button));
        simulate(&rdev::EventType::ButtonRelease(button));
        std::thread::sleep(Duration::from_millis(delay));

        match rx.lock().unwrap().try_recv() {
            Ok(_) | Err(TryRecvError::Disconnected) => break,
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
    let mut flex = Flex::default();
    Frame::default()
        .with_label(label)
        .with_align(Align::Inside | Align::Right);
    let widget = func();
    flex.set_size(&widget, 100);
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
        .with_size(300, 300)
        .with_label("Auto clicker")
        .center_screen();

    // Layout
    let flex = Flex::default()
        .with_size(100, 150)
        .column()
        .center_of_parent();

    Frame::default().with_label("Auto clicker");

    let mut delay_ipt = with_label(IntInput::default, "Delay: ");
    delay_ipt.set_value("100");

    let mut keybind_btn = with_label(Button::default, "Keybind: ");

    let mut button_select = with_label(Choice::default, "Button: ");
    button_select.add_choice("Left");
    button_select.add_choice("Right");
    button_select.add_choice("Middle");
    button_select.set_value(0);

    let mut clicking_text = Frame::default();

    flex.end();

    window.end();
    window.show();

    // Logic
    let (s, r) = app::channel::<Message>();

    // When loop_tx sends anything, the loop clickin  thread will stop
    let (loop_tx, loop_rx) = mpsc::channel();
    let loop_rx = Arc::new(Mutex::new(loop_rx));

    // Listening to rdev events blocks the thread so move it to different one
    std::thread::spawn(move || {
        rdev::listen(move |event| handle_rdev_event(&event, &s)).unwrap();
    });

    let mut is_clicking = false;
    let mut keybind = None;
    let mut is_setting_keybind = false;

    keybind_btn.set_callback(move |_| {
        // Need to send messages because can't modify variables in callback
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
                        clicking_text.set_label("");
                    } else {
                        let button = match button_select.value() {
                            0 => rdev::Button::Left,
                            1 => rdev::Button::Right,
                            2 => rdev::Button::Middle,
                            _ => unreachable!(),
                        };

                        let delay = delay_ipt.value().parse().unwrap();
                        start_clicking(loop_rx.clone(), delay, button);
                        is_clicking = true;
                        clicking_text.set_label("Clicking...");
                    }
                }
                // Key press messages sent from rdev thread
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
