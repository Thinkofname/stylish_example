//! UI system for the game.
//!
//! The format is documented in the `format` package.

mod layout;

use sdl2::keyboard::Keycode;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use stylish;
use stylish_webrender;

/// The stylish node type used
pub type Node = stylish::Node<stylish_webrender::Info>;
/// The weak version of the stylish node type used
pub type WeakNode = stylish::WeakNode<stylish_webrender::Info>;


/// Manages all UI elements.
pub struct Manager {
    /// The stylish ui manager
    pub manager: Rc<RefCell<stylish::Manager<stylish_webrender::Info>>>,

    current_focus: Option<WeakNode>,
    last_hover: Option<WeakNode>,

    style_groups: HashMap<String, Vec<String>>,

    // Used for init/deinit checking
    cycle: bool,
    nodes: Vec<Node>,

    events: Vec<NodeEvent>,
}

fn list(params: Vec<stylish::Value>) -> stylish::SResult<stylish::Value> {
    Ok(stylish::Value::Any(Box::new(params)))
}

impl Manager {
    pub fn new() -> Manager {
        Manager {
            manager: Rc::new(RefCell::new({
                let mut manager = stylish::Manager::new();
                manager.add_func_raw("list", list);
                manager.add_layout_engine("center", |_| Box::new(layout::Center));
                manager.add_layout_engine("padded", |o| Box::new(layout::Padded::new(o)));
                manager.add_layout_engine("rows", |o| Box::new(layout::Rows::new(o)));
                manager.add_layout_engine("clipped", |_| Box::new(layout::Clipped));
                manager.add_layout_engine("push_bottom", |_| Box::new(layout::PushBottom));

                manager
            })),

            current_focus: None,
            last_hover: None,

            style_groups: HashMap::new(),

            cycle: false,
            nodes: Vec::new(),

            events: Vec::new(),
        }
    }

    pub fn events(&mut self) -> ::std::vec::Drain<NodeEvent> {
        self.events.drain(..)
    }

    /// Handles text boxes
    pub fn update(&mut self, delta: f64) -> Option<stylish::Rect> {
        let mut text_area = None;
        for node in self.manager.borrow().query()
            .property("focused", true)
            .matches()
        {
            if node.get_value::<stylish::Value>("on_char_input").is_some() {
                if let Some(rect) = node.render_position() {
                    text_area = Some(rect);
                }
                break;
            }
        }

        self.cycle = !self.cycle;

        for node in self.manager.borrow().query().matches() {
            node.raw_set_property("$cycle", self.cycle);
            if node.has_layout() && node.get_property::<bool>("$init").is_none() {
                node.raw_set_property("$init", true);
                if let Some(method) = node.get_value("on_init") {
                    self.events.push(NodeEvent {
                        node: node.clone(),
                        ty: EventType::Init,
                        value: method,
                    });
                }
                self.nodes.push(node.clone());
            }
            if let Some(method) = node.get_value("on_update") {
                self.events.push(NodeEvent {
                    node: node.clone(),
                    ty: EventType::Update(delta),
                    value: method,
                });
            }
        }

        let cycle = self.cycle;
        let events = &mut self.events;
        self.nodes.retain(|v| {
            if v.get_property::<bool>("$cycle").map_or(true, |c| c != cycle) {
                if v.get_property::<bool>("$init").is_some() {
                    if let Some(method) = v.get_value("on_deinit") {
                        events.push(NodeEvent {
                            node: v.clone(),
                            ty: EventType::Deinit,
                            value: method,
                        });
                    }
                }
                false
            } else {
                true
            }
        });

        text_area
    }

    /// Loads the named style rules
    pub fn load_styles(&mut self, key: &str) {
        use std::io::Read;
        use std::io::stdout;
        use std::fs::File;

        // Save repeated derefs
        let manager: &mut stylish::Manager<_> = &mut *self.manager.borrow_mut();

        // Remove the old styles in this group if they exist
        for old in self.style_groups.remove(key).into_iter().flat_map(|v| v) {
            manager.remove_styles(&old);
        }

        let mut group = Vec::new();
        let mut styles = String::new();
        let mut res = if let Ok(res) = File::open(format!("styles/{}.list", key)) {
            res
        } else {
            // Missing file, ignore
            return
        };
        res.read_to_string(&mut styles).unwrap();
        for line in styles.lines() {
            let line = line.trim();
            // Skip empty lines/comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            group.push(line.to_owned());

            let mut style = String::new();
            let mut res = File::open(format!("styles/{}.style", line)).unwrap();
            res.read_to_string(&mut style).unwrap();
            // Instead of failing on error just report it in the console.
            // TODO: Maybe report on screen somewhere?
            if let Err(err) = manager.load_styles(&line, &style) {
                println!("Failed to parse {:?}", line);
                stylish::format_parse_error(
                    stdout(),
                    style.lines(),
                    err,
                ).unwrap();
            }
        }

        self.style_groups.insert(key.to_owned(), group);
    }

    /// Renames the named style rules
    pub fn remove_styles(&self, name: &str) {
        self.manager.borrow_mut().remove_styles(name);
    }

    /// Loads and adds the node as described by the resource.
    pub fn create_node(&self, key: &str) -> Node {
        use std::io::Read;
        use std::fs::File;
        let mut desc = String::new();
        let mut res = File::open(format!("ui/{}.desc", key)).unwrap();
        res.read_to_string(&mut desc).unwrap();
        let node = Node::from_str(&desc).unwrap();
        self.manager.borrow_mut().add_node(node.clone());
        node
    }

    /// Adds the passed node to the root node
    pub fn add_node(&self, node: Node) {
        self.manager.borrow_mut().add_node(node);
    }

    /// Removes the passed node from the root node
    pub fn remove_node(&self, node: Node) {
        self.manager.borrow_mut().remove_node(node);
    }

    /// Handles events targetting the focused element
    pub fn focused_event<E>(&mut self, param: E::Param) -> bool
        where E: Event + 'static,
    {
        if let Some(node) = self.current_focus.as_ref().and_then(|v| v.upgrade()) {
            if let Some(method) = node.get_value(E::event_key()) {
                self.events.push(NodeEvent {
                    node: node.clone(),
                    ty: E::into_node_event(param),
                    value: method,
                });
                return true;
            }
        }
        false
    }

    /// Handles mouse move events
    pub fn mouse_event<E>(&mut self, x: i32, y: i32, param: E::Param) -> bool
        where E: Event + 'static,
    {
        let matches = {
            let manager = self.manager.borrow();
            manager.query_at(x, y).matches()
        };
        for node in matches {
            if let Some(method) = node.get_value(E::event_key()) {
                self.events.push(NodeEvent {
                    node: node.clone(),
                    ty: E::into_node_event(param),
                    value: method,
                });
                return true;
            }
        }
        false
    }

    /// Handles mouse move events
    pub fn mouse_move(&mut self, x: i32, y: i32) -> bool {
        let matches = {
            let manager = self.manager.borrow();
            manager.query_at(x, y).matches()
        };
        for node in matches {
            if node.get_value::<bool>("can_hover").unwrap_or(false) {
                if self.last_hover.as_ref()
                    .and_then(|v| v.upgrade())
                    .map_or(true, |v| !v.is_same(&node))
                {
                    if let Some(last_hover) = self.last_hover.take()
                        .and_then(|v| v.upgrade())
                    {
                        last_hover.set_property("hover", false);
                        if let Some(method) = last_hover.get_value("on_mouse_move_out") {
                            self.events.push(NodeEvent {
                                node: last_hover.clone(),
                                ty: MouseMoveEvent::into_node_event(MouseMove {
                                    x: x,
                                    y: y,
                                }),
                                value: method,
                            });
                        }
                    }
                    node.set_property("hover", true);
                    self.last_hover = Some(node.weak());
                    if let Some(method) = node.get_value("on_mouse_move_over") {
                        self.events.push(NodeEvent {
                            node: node.clone(),
                            ty: MouseMoveEvent::into_node_event(MouseMove {
                                x: x,
                                y: y,
                            }),
                            value: method,
                        });
                    }
                }
                if let Some(method) = node.get_value("on_mouse_move") {
                    self.events.push(NodeEvent {
                        node: node.clone(),
                        ty: MouseMoveEvent::into_node_event(MouseMove {
                            x: x,
                            y: y,
                        }),
                        value: method,
                    });
                }
                return true;
            }
        }
        if let Some(last_hover) = self.last_hover.take()
            .and_then(|v| v.upgrade())
        {
            last_hover.set_property("hover", false);
            if let Some(method) = last_hover.get_value("on_mouse_move_out") {
                self.events.push(NodeEvent {
                    node: last_hover.clone(),
                    ty: MouseMoveEvent::into_node_event(MouseMove {
                        x: x,
                        y: y,
                    }),
                    value: method,
                });
            }
        }
        false
    }

    /// Focuses the passed node
    pub fn focus_node(&mut self, node: Node) {
        if let Some(current) = self.current_focus
            .as_ref()
            .and_then(|v| v.upgrade())
        {
            current.set_property("focused", false);
            if let Some(method) = current.get_value("on_unfocus") {
                self.events.push(NodeEvent {
                    node: current.clone(),
                    ty: EventType::Unfocus,
                    value: method,
                });
            }
        }
        self.current_focus = Some(node.weak());
        node.set_property("focused", true);
        if let Some(method) = node.get_value("on_focus") {
            self.events.push(NodeEvent {
                node: node.clone(),
                ty: EventType::Focus,
                value: method,
            });
        }
    }

    /// Cycles the focus to the next element that can take input
    /// if one exists
    pub fn cycle_focus(&mut self) {
        let manager = self.manager.borrow();
        let mut current = self.current_focus
            .as_ref()
            .and_then(|v| v.upgrade());

        let matches = manager.query()
            .matches()
            .collect::<Vec<_>>();
        let mut can_loop = true;
        while can_loop {
            can_loop = false;
            for node in matches.iter().rev() {
                if current.as_ref().map_or(false, |v| v.is_same(node)) {
                    node.set_property("focused", false);
                    if let Some(method) = node.get_value("on_unfocus") {
                        self.events.push(NodeEvent {
                            node: node.clone(),
                            ty: EventType::Unfocus,
                            value: method,
                        });
                    }
                    current = None;
                    can_loop = true;
                } else if current.is_none() && node.get_value::<bool>("can_focus").unwrap_or(false) {
                    node.set_property("focused", true);
                    if let Some(method) = node.get_value("on_focus") {
                        self.events.push(NodeEvent {
                            node: node.clone(),
                            ty: EventType::Focus,
                            value: method,
                        });
                    }
                    self.current_focus = Some(node.weak());
                    can_loop = false;
                    break;
                }
            }
            if current.is_some() {
                current = None;
                can_loop = true;
            }
        }
    }
}

// Events

/// References a button on the mouse
#[derive(Clone, Copy, Debug)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button
    Middle,
    /// Unknown mouse button
    Unknown,
}

impl From<::sdl2::mouse::MouseButton> for MouseButton {
    fn from(m: ::sdl2::mouse::MouseButton) -> MouseButton {
        use ::sdl2::mouse::MouseButton::*;
        match m {
            Left => MouseButton::Left,
            Right => MouseButton::Right,
            Middle => MouseButton::Middle,
            _ => MouseButton::Unknown,
        }
    }
}

/// The parameter to mouse click events
#[derive(Clone, Copy, Debug)]
pub struct MouseClick {
    /// The mouse button pressed if any
    pub button: MouseButton,
    /// The x position of the mouse
    pub x: i32,
    /// The y position of the mouse
    pub y: i32,
}

/// The parameter to mouse move events
#[derive(Clone, Copy, Debug)]
pub struct MouseMove {
    /// The x position of the mouse
    pub x: i32,
    /// The y position of the mouse
    pub y: i32,
}

/// The parameter to mouse scroll events
#[derive(Clone, Copy, Debug)]
pub struct MouseScroll {
    /// The x position of the mouse
    pub x: i32,
    /// The y position of the mouse
    pub y: i32,
    /// The amount the mouse wheel was scrolled by
    pub scroll_amount: i32,
}

/// Parameter to events that invoke a character being
/// input.
#[derive(Clone, Copy, Debug)]
pub struct CharInput {
    /// The input character
    pub input: char,
}

/// Parameter to events that invoke a key being
/// pressed.
#[derive(Clone, Copy, Debug)]
pub struct KeyInput {
    /// The input key
    pub input: Keycode,
}

/// Event that is fired when the mouse moves
pub enum MouseMoveEvent {}

impl Event for MouseMoveEvent {
    type Param = MouseMove;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::MouseMove(p)
    }

    fn event_key() -> &'static str {
        "on_mouse_move"
    }
}

/// Event that is fired when a mouse button is pressed
pub enum MouseDownEvent {}

impl Event for MouseDownEvent {
    type Param = MouseClick;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::MouseDown(p)
    }

    fn event_key() -> &'static str {
        "on_mouse_down"
    }
}

/// Event that is fired when a mouse button is released
pub enum MouseUpEvent {}

impl Event for MouseUpEvent {
    type Param = MouseClick;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::MouseUp(p)
    }

    fn event_key() -> &'static str {
        "on_mouse_up"
    }
}

/// Event that is fired when the mouse wheel is scrolled
pub enum MouseScrollEvent {}

impl Event for MouseScrollEvent {
    type Param = MouseScroll;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::MouseScroll(p)
    }

    fn event_key() -> &'static str {
        "on_mouse_scroll"
    }
}

/// Event that is fired when character is input
pub enum CharInputEvent {}

impl Event for CharInputEvent {
    type Param = CharInput;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::CharInput(p)
    }

    fn event_key() -> &'static str {
        "on_char_input"
    }
}

/// Event that is fired when a key is pressed
pub enum KeyDownEvent {}

impl Event for KeyDownEvent {
    type Param = KeyInput;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::KeyDown(p)
    }

    fn event_key() -> &'static str {
        "on_key_down"
    }
}

/// Event that is fired when a key is released
pub enum KeyUpEvent {}

impl Event for KeyUpEvent {
    type Param = KeyInput;

    fn into_node_event(p: Self::Param) -> EventType {
        EventType::KeyUp(p)
    }

    fn event_key() -> &'static str {
        "on_key_up"
    }
}

/// An event that can be handled by an element
pub trait Event: Sized {
    /// The parameter to pass to the handler
    type Param;

    fn into_node_event(p: Self::Param) -> EventType;
    fn event_key() -> &'static str;
}

#[derive(Clone, Copy, Debug)]
pub enum EventType {
    Init,
    Deinit,
    Update(f64),
    Focus,
    Unfocus,
    KeyUp(KeyInput),
    KeyDown(KeyInput),
    CharInput(CharInput),
    MouseScroll(MouseScroll),
    MouseUp(MouseClick),
    MouseDown(MouseClick),
    MouseMove(MouseMove),
}

#[derive(Clone)]
pub struct NodeEvent {
    pub node: Node,
    pub ty: EventType,
    pub value: String,
}

impl ::std::fmt::Debug for NodeEvent {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "NodeEvent {{ {:?}, {:?} for {:?} }}", self.ty, self.value, self.node.name())
    }
}