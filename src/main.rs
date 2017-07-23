
#[macro_use]
extern crate stylish;
extern crate stylish_webrender;
extern crate sdl2;
extern crate chrono;

pub mod ui;
use ui::EventType;
pub mod assets;

use std::time::{Duration, Instant};
use std::thread;
use sdl2::keyboard::Keycode;
use sdl2::event::Event;
use chrono::prelude::*;

fn main() {
    let sdl = sdl2::init()
        .expect("Failed to initialize SDL2");
    let video = sdl.video()
        .expect("Failed to create a video backend");

    let gl_attr = video.gl_attr();
    gl_attr.set_stencil_size(8);
    gl_attr.set_depth_size(24);
    gl_attr.set_context_major_version(3);
    gl_attr.set_context_minor_version(2);
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    let mut window = video.window("Discord-Rust", 1024, 640)
        .position_centered()
        .opengl()
        .resizable()
        .build()
        .expect("Failed to open a window");
    window.maximize();

    let mut sdl_events = sdl.event_pump()
        .expect("Failed to get the event pump");
    let input = video.text_input();

    let gl_context = window.gl_create_context().expect("Failed to create opengl context");
    window.gl_make_current(&gl_context).expect("Could not set current context.");

    let mut ui_manager = ui::Manager::new();
    ui_manager.load_styles("base");
    let root = ui_manager.create_node("main");

    let mut ui_renderer = stylish_webrender::WebRenderer::new(
        |n| video.gl_get_proc_address(n),
        assets::AssetLoader,
        &mut *ui_manager.manager.borrow_mut(),
    )
        .unwrap();

    let mut last_frame = Instant::now();
    let mut last_rect = None;
    let mut mouse_pos = (0, 0);

    ui_renderer.layout(&mut *ui_manager.manager.borrow_mut(), 0, 0);

    loop {
        let start = Instant::now();
        let diff = last_frame.elapsed();
        last_frame = start;
        let delta =
            (diff.as_secs() * 1_000_000_000 + diff.subsec_nanos() as u64) as f64 / (1_000_000_000.0 / 60.0);

        let (width, height) = window.drawable_size();

        for sdlevent in sdl_events.poll_iter() {
            match sdlevent {
                Event::TextInput{ref text, ..} => {
                    for c in text.chars() {
                        ui_manager.focused_event::<ui::CharInputEvent>(ui::CharInput {
                            input: c,
                        });
                    }
                },
                Event::MouseMotion{x, y, ..} => {
                    mouse_pos = (x, y);
                    ui_manager.mouse_move(x, y);
                },
                Event::MouseButtonDown{x, y, mouse_btn, ..} => {
                    ui_manager.mouse_event::<ui::MouseDownEvent>(
                        x, y,
                        ui::MouseClick { button: mouse_btn.into(), x: x, y: y},
                    );
                }
                Event::MouseButtonUp{x, y, mouse_btn, ..} => {
                    ui_manager.mouse_event::<ui::MouseUpEvent>(
                        x, y,
                        ui::MouseClick { button: mouse_btn.into(), x: x, y: y},
                    );
                }
                Event::MouseWheel{y, ..} => {
                    ui_manager.mouse_event::<ui::MouseScrollEvent>(
                        mouse_pos.0,
                        mouse_pos.1,
                        ui::MouseScroll {
                            x: mouse_pos.0,
                            y: mouse_pos.1,
                            scroll_amount: y
                        },
                    );
                },
                Event::KeyDown{scancode: Some(sdl2::keyboard::Scancode::Grave), ..} => {
                    ui_manager.load_styles("base");
                },
                Event::KeyUp{scancode: Some(sdl2::keyboard::Scancode::Grave), ..} => {

                },
                Event::KeyDown{keycode: Some(Keycode::Tab), ..} => {
                    ui_manager.cycle_focus();
                },
                Event::KeyUp{keycode: Some(key), ..} => {
                    ui_manager.focused_event::<ui::KeyUpEvent>(ui::KeyInput {
                        input: key
                    });
                },
                Event::KeyDown{keycode: Some(key), ..} => {
                    ui_manager.focused_event::<ui::KeyDownEvent>(ui::KeyInput {
                        input: key
                    });
                },
                Event::Quit{..} => {
                    return;
                },
                _ => {},
            }
        }

        if let Some(r) = ui_manager.update(delta) {
            if last_rect != Some(r) {
                if !input.is_active() {
                    input.start();
                }
                input.set_rect(sdl2::rect::Rect::new(r.x, r.y, r.width as u32, r.height as u32));
                last_rect = Some(r)
            }
        } else {
            if input.is_active() {
                input.stop();
            }
            last_rect = None;
        }

        let mut new_focus = None;
        for event in ui_manager.events() {
            let ui::NodeEvent{value, ty, node} = event;
            match (value.as_str(), ty, node) {
                ("focus", _, node) => {
                    new_focus = Some(node);
                },
                ("textbox", EventType::Focus, node) => {
                    node.set_property("$tb_info", TextboxInfo {
                        backspace_timer: 40.0,
                        backspace_first: true,
                        cursor_timer: 30.0,
                        cursor: None,
                        deleting: false,
                    })
                },
                ("textbox", EventType::Unfocus, node) => {
                    let mut info: TextboxInfo = node.get_custom_property("$tb_info").unwrap();
                    if let Some(cursor) = info.cursor.take() {
                        let content = query!(node, content).next().unwrap();
                        content.remove_child(cursor);
                        node.set_property("$tb_info", info);
                    }
                },
                ("textbox", EventType::Update(delta), node) => {
                    if node.get_property::<bool>("focused").unwrap_or(false) {
                        let txt = query!(node, @text).next().unwrap();
                        let mut info: TextboxInfo = node.get_custom_property("$tb_info").unwrap();
                        if info.deleting {
                            if info.backspace_first {
                                info.backspace_first = false;
                                let mut text = txt.text().unwrap();
                                text.pop();
                                txt.set_text(text);
                            }
                            info.backspace_timer -= delta;
                            if info.backspace_timer <= 0.0 {
                                let mut text = txt.text().unwrap();
                                text.pop();
                                txt.set_text(text);
                                info.backspace_timer = 5.0;
                            }
                        } else {
                            info.backspace_timer = 40.0;
                            info.backspace_first = true;
                        }

                        info.cursor_timer -= delta;
                        if info.cursor_timer <= 0.0 {
                            let content = query!(node, content).next().unwrap();
                            if let Some(cursor) = info.cursor.take() {
                                content.remove_child(cursor);
                            } else {
                                let node = node!(cursor);
                                info.cursor = Some(node.clone());
                                content.add_child(node);
                            }
                            info.cursor_timer = 30.0;
                        }

                        node.set_property("$tb_info", info);
                    }
                },
                ("textbox", EventType::KeyDown(evt), node) => {
                    if evt.input == Keycode::Backspace {
                        let mut info: TextboxInfo = node.get_custom_property("$tb_info").unwrap();
                        info.deleting = true;
                        node.set_property("$tb_info", info);
                    }
                },
                ("textbox", EventType::KeyUp(evt), node) => {
                    if evt.input == Keycode::Backspace {
                        let mut info: TextboxInfo = node.get_custom_property("$tb_info").unwrap();
                        info.deleting = false;
                        node.set_property("$tb_info", info);
                    } else if evt.input == Keycode::Return {
                        let txt = query!(node, @text).next().unwrap();
                        let text = txt.text().unwrap();
                        txt.set_text("");
                        let messages = query!(root, chat_area > content).next().unwrap();

                        let time = Local::now();

                        let msg = node!{
                            message {
                                icon
                                author {
                                    @text(time.format(" Today at %H:%M").to_string())
                                }
                                content {
                                    @text(text)
                                }
                            }
                        };
                        let author = query!(msg, author).next().unwrap();
                        author.add_child_first({
                            let t = ui::Node::new_text("Rust User");
                            t.set_property("col", "#FFBF00".to_owned());
                            t
                        });
                        messages.add_child(msg);
                    }
                },
                ("textbox", EventType::CharInput(evt), node) => {
                    let txt = query!(node, @text).next().unwrap();
                    let mut text = txt.text().unwrap();
                    text.push(evt.input);
                    txt.set_text(text);
                }
                event => println!("{:?} {:?}", event.0, event.1),
            }
        }

        if let Some(focus) = new_focus {
            ui_manager.focus_node(focus);
        }

        ui_renderer.layout(&mut *ui_manager.manager.borrow_mut(), width, height);
        ui_renderer.render(&mut *ui_manager.manager.borrow_mut(), width, height);

        window.gl_swap_window();
        // Keep the game at our target fps. TODO: Make config option
        let frame_time = start.elapsed();
        let target = 60;

        if target != i32::max_value() as u32 {
            let target_frame_time = Duration::from_secs(1) / target;
            if frame_time < target_frame_time {
                thread::sleep(target_frame_time - frame_time);
            }
        }
    }
}

#[derive(Clone)]
struct TextboxInfo {
    backspace_timer: f64,
    backspace_first: bool,
    cursor_timer: f64,
    cursor: Option<ui::Node>,
    deleting: bool,
}
impl stylish::CustomValue for TextboxInfo {
    fn clone(&self) -> Box<stylish::CustomValue> {
        Box::new(Clone::clone(self))
    }
}